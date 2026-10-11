//! The engine: an in-memory vault index driven by generation-tagged requests.
//!
//! The host owns the generation counter (EXP-06). Every mutation carries a
//! generation strictly greater than the engine's, except [`Request::Load`],
//! which replaces the whole index (used for the first load and for replay
//! into a respawned worker) and adopts whatever generation it carries. Every
//! read carries the generation the host believes is current; a mismatch is
//! answered `stale` without doing the work. A host therefore never sees a
//! result computed against a superseded source.
//!
//! The index is derived state: documents are held losslessly
//! ([`ontomark_parse::Document`]), the projection is recomputed from them on
//! demand (full rebuild is the correctness oracle, ADR-2016), and nothing is
//! persisted. The engine never writes a file: [`Request::ProposePatch`]
//! returns the guarded new text for the host to apply after review.

use std::collections::BTreeMap;

use ontomark_parse::{Document, Patch, PatchOp};
use oxrdf::vocab::{rdf, rdfs};
use oxrdf::{NamedNode, NamedOrBlankNode, Term, Triple};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::profile::ProfileId;
use crate::projection::{self, owl, Compiled};
use crate::query::{self, DatasetSelection, WireTerm};
use crate::reason::{Classification, Progress, Reasoner, ReasonerRun, WhelkEl};
use crate::store::Store;
use crate::text::{unified_diff, HostSpan, TripleSet};

/// Default number of reasoner work units per classify step.
/// One slice is about 25 ms of saturation natively (WP-B measurement).
pub const DEFAULT_STEP_BUDGET: u64 = 20_000;

/// A vault file as the host read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileText {
    /// Vault-relative path, `/`-separated, with extension.
    pub path: String,
    /// Exact file contents.
    pub text: String,
}

/// A request from the host. The JSON form is tagged by `kind` (snake case).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// Replace the whole index with `files` (Markdown only; others are ignored).
    Load {
        /// Generation the host assigns to this state.
        generation: u64,
        /// Profile to index under.
        #[serde(default)]
        profile: ProfileId,
        /// The vault's files.
        files: Vec<FileText>,
    },
    /// Create or replace one file.
    Upsert {
        /// New generation.
        generation: u64,
        /// Path.
        path: String,
        /// Exact contents.
        text: String,
    },
    /// Delete one file.
    Remove {
        /// New generation.
        generation: u64,
        /// Path.
        path: String,
    },
    /// Rename one file, keeping its contents.
    Rename {
        /// New generation.
        generation: u64,
        /// Old path.
        from: String,
        /// New path.
        to: String,
    },
    /// Switch profile.
    SetProfile {
        /// New generation.
        generation: u64,
        /// Profile.
        profile: ProfileId,
    },
    /// Entities, counts and diagnostics.
    Index {
        /// Current generation.
        generation: u64,
    },
    /// One entity and its incoming and outgoing asserted triples.
    Describe {
        /// Current generation.
        generation: u64,
        /// Entity IRI.
        iri: String,
    },
    /// Start classification and do the first step.
    ClassifyStart {
        /// Current generation.
        generation: u64,
        /// Work units per step.
        #[serde(default)]
        budget: Option<u64>,
    },
    /// Continue the running classification by one step.
    ClassifyStep {
        /// Current generation.
        generation: u64,
        /// Work units for this step.
        #[serde(default)]
        budget: Option<u64>,
    },
    /// Classify to completion in one call.
    Classify {
        /// Current generation.
        generation: u64,
    },
    /// One page's parsed frontmatter (read-only), for building patch operations.
    Frontmatter {
        /// Current generation.
        generation: u64,
        /// Path.
        path: String,
    },
    /// Run a read-only SPARQL query.
    Query {
        /// Current generation.
        generation: u64,
        /// Query text.
        sparql: String,
        /// Default graph selection.
        #[serde(default)]
        dataset: DatasetSelection,
        /// Row cap.
        #[serde(default)]
        limit: Option<usize>,
    },
    /// Plan a guarded patch to one file; never writes.
    ProposePatch {
        /// Current generation.
        generation: u64,
        /// Path.
        path: String,
        /// Digest (`sha256:<hex>`) of the text the host showed the user.
        base_digest: String,
        /// Operations (the `ontomark-parse` wire form).
        ops: Vec<PatchOp>,
    },
    /// Engine and reasoner identity.
    Version,
}

/// A wire error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    /// Stable machine code (`stale`, `generation_regressed`, `query_syntax`, …).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// A response. `generation` is always the engine's generation after handling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// `true` when `result` is present.
    pub ok: bool,
    /// The engine's generation.
    pub generation: u64,
    /// The result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Json>,
    /// The error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}

struct Classified {
    classification: Classification,
    inferred: Vec<Triple>,
}

/// The engine. See the module documentation.
pub struct Engine {
    generation: u64,
    profile: ProfileId,
    docs: BTreeMap<String, Document>,
    compiled: Option<Compiled>,
    reasoner: Box<dyn Reasoner>,
    run: Option<Box<dyn ReasonerRun>>,
    classified: Option<Classified>,
    /// Query stores built for this generation, keyed by (selection, named graphs).
    datasets: Vec<(DatasetSelection, bool, Store)>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// An empty engine at generation 0 with the generic profile and the EL reasoner.
    #[must_use]
    pub fn new() -> Self {
        Self::with_reasoner(Box::new(WhelkEl::default()))
    }

    /// An empty engine using `reasoner`.
    #[must_use]
    pub fn with_reasoner(reasoner: Box<dyn Reasoner>) -> Self {
        Self {
            generation: 0,
            profile: ProfileId::Generic,
            docs: BTreeMap::new(),
            compiled: None,
            reasoner,
            run: None,
            classified: None,
            datasets: Vec::new(),
        }
    }

    /// The current generation.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Handle a JSON request and return the JSON response. Never panics on bad input.
    #[must_use]
    pub fn handle_json(&mut self, request: &str) -> String {
        let mut response = match serde_json::from_str::<Request>(request) {
            Ok(req) => self.handle(req),
            Err(e) => self.err("bad_request", e.to_string()),
        };
        // The compiler enables serde_json's preserve_order feature. Sort nested
        // results so feature unification cannot change the native/WASM protocol.
        if let Some(result) = &mut response.result {
            result.sort_all_objects();
        }
        serde_json::to_string(&response).unwrap_or_else(|e| {
            format!(
                r#"{{"ok":false,"generation":{},"error":{{"code":"internal","message":"{e}"}}}}"#,
                self.generation
            )
        })
    }

    /// Handle a request.
    pub fn handle(&mut self, request: Request) -> Response {
        match request {
            Request::Load {
                generation,
                profile,
                files,
            } => {
                self.docs = files
                    .into_iter()
                    .filter(|f| is_markdown(&f.path))
                    .map(|f| (f.path, Document::parse(f.text)))
                    .collect();
                self.profile = profile;
                self.generation = generation;
                self.invalidate();
                self.ok(json!({ "documents": self.docs.len() }))
            }
            Request::Upsert {
                generation,
                path,
                text,
            } => self.mutate(generation, |e| {
                if is_markdown(&path) {
                    e.docs.insert(path, Document::parse(text));
                } else {
                    e.docs.remove(&path);
                }
            }),
            Request::Remove { generation, path } => self.mutate(generation, |e| {
                e.docs.remove(&path);
            }),
            Request::Rename {
                generation,
                from,
                to,
            } => self.mutate(generation, |e| {
                if let Some(doc) = e.docs.remove(&from) {
                    if is_markdown(&to) {
                        e.docs.insert(to, doc);
                    }
                }
            }),
            Request::SetProfile {
                generation,
                profile,
            } => self.mutate(generation, |e| e.profile = profile),
            Request::Version => self.ok(json!({
                "crate": "ontomark-wasm",
                "version": env!("CARGO_PKG_VERSION"),
                "reasoner": self.reasoner.info(),
                "profiles": [ProfileId::Generic, ProfileId::Estate],
            })),
            Request::Index { generation } => self.read(generation, Self::index),
            Request::Describe { generation, iri } => self.read(generation, |e| e.describe(&iri)),
            Request::ClassifyStart { generation, budget } => self.read(generation, |e| {
                let axioms = e.compiled().axioms.clone();
                e.run = Some(e.reasoner.start(&axioms));
                e.classified = None;
                e.step(budget)
            }),
            Request::ClassifyStep { generation, budget } => {
                self.read(generation, |e| e.step(budget))
            }
            Request::Classify { generation } => self.read(generation, |e| {
                let axioms = e.compiled().axioms.clone();
                e.run = Some(e.reasoner.start(&axioms));
                e.classified = None;
                e.step(Some(u64::MAX))
            }),
            Request::Frontmatter { generation, path } => {
                self.read(generation, |e| e.frontmatter(&path))
            }
            Request::Query {
                generation,
                sparql,
                dataset,
                limit,
            } => self.read(generation, |e| e.query(&sparql, dataset, limit)),
            Request::ProposePatch {
                generation,
                path,
                base_digest,
                ops,
            } => self.read(generation, |e| e.propose(&path, &base_digest, ops)),
        }
    }

    fn ok(&self, result: Json) -> Response {
        Response {
            ok: true,
            generation: self.generation,
            result: Some(result),
            error: None,
        }
    }

    fn err(&self, code: &str, message: impl Into<String>) -> Response {
        Response {
            ok: false,
            generation: self.generation,
            result: None,
            error: Some(WireError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    fn invalidate(&mut self) {
        self.compiled = None;
        self.run = None;
        self.classified = None;
        self.datasets.clear();
    }

    fn mutate(&mut self, generation: u64, f: impl FnOnce(&mut Self)) -> Response {
        if generation <= self.generation {
            return self.err(
                "generation_regressed",
                format!(
                    "mutation generation {generation} is not after the engine's {}",
                    self.generation
                ),
            );
        }
        f(self);
        self.generation = generation;
        self.invalidate();
        self.ok(json!({ "documents": self.docs.len() }))
    }

    fn read(
        &mut self,
        generation: u64,
        f: impl FnOnce(&mut Self) -> Result<Json, WireError>,
    ) -> Response {
        if generation != self.generation {
            return self.err(
                "stale",
                format!(
                    "request generation {generation}, engine generation {}",
                    self.generation
                ),
            );
        }
        match f(self) {
            Ok(v) => self.ok(v),
            Err(e) => Response {
                ok: false,
                generation: self.generation,
                result: None,
                error: Some(e),
            },
        }
    }

    fn compiled(&mut self) -> &Compiled {
        if self.compiled.is_none() {
            self.compiled = Some(projection::compile(&self.docs, self.profile.profile()));
        }
        self.compiled
            .as_ref()
            .unwrap_or_else(|| unreachable!("compiled just above"))
    }

    fn index(&mut self) -> Result<Json, WireError> {
        let profile = self.profile;
        let documents = self.docs.len();
        let reasoner = self.reasoner.info();
        let classification = self.classified.as_ref().map(|c| c.classification.clone());
        let c = self.compiled();
        Ok(json!({
            "profile": profile,
            "keys": {
                "type": profile.profile().type_key,
                "iri": profile.profile().iri_key,
                "subclass": profile.profile().subclass_keys[0],
                "instance": profile.profile().instance_keys[0],
            },
            "documents": documents,
            "entities": c.entities,
            "triples": c.triples.len(),
            "axioms": c.axioms.len(),
            "diagnostics": c.diagnostics,
            "reasoner": reasoner,
            "classification": classification,
        }))
    }

    fn frontmatter(&self, path: &str) -> Result<Json, WireError> {
        let doc = self
            .docs
            .get(path)
            .ok_or_else(|| wire("not_found", format!("no document {path}")))?;
        let source = doc.source();
        let (state, entries) = match doc.frontmatter() {
            None => (json!("absent"), Vec::new()),
            Some(fm) => {
                let entries = fm
                    .entries
                    .iter()
                    .map(|e| {
                        json!({
                            "key": e.key,
                            "value": e.value,
                            "span": HostSpan::of(source, e.span.start, e.span.end),
                        })
                    })
                    .collect();
                let state =
                    serde_json::to_value(&fm.state).map_err(|e| wire("internal", e.to_string()))?;
                (state, entries)
            }
        };
        Ok(
            json!({ "path": path, "digest": doc.digest().to_string(), "state": state, "entries": entries }),
        )
    }

    fn describe(&mut self, iri: &str) -> Result<Json, WireError> {
        let c = self.compiled();
        let entity = c
            .entity(iri)
            .cloned()
            .ok_or_else(|| wire("not_found", format!("no entity {iri}")))?;
        let node = Term::NamedNode(NamedNode::new_unchecked(iri));
        let as_wire = |t: &Triple| {
            [
                WireTerm::from(&Term::from(t.subject.clone())),
                WireTerm::from(&Term::NamedNode(t.predicate.clone())),
                WireTerm::from(&t.object),
            ]
        };
        let outgoing: Vec<_> = c
            .triples
            .iter()
            .filter(|t| Term::from(t.subject.clone()) == node)
            .map(as_wire)
            .collect();
        let incoming: Vec<_> = c
            .triples
            .iter()
            .filter(|t| t.object == node)
            .map(as_wire)
            .collect();
        Ok(json!({ "entity": entity, "outgoing": outgoing, "incoming": incoming }))
    }

    fn step(&mut self, budget: Option<u64>) -> Result<Json, WireError> {
        let run = self
            .run
            .as_mut()
            .ok_or_else(|| wire("no_run", "no classification is running for this generation"))?;
        match run.step(budget.unwrap_or(DEFAULT_STEP_BUDGET)) {
            Progress::Pending { steps, total } => {
                Ok(json!({ "status": "pending", "steps": steps, "total": total }))
            }
            Progress::Done(classification) => {
                self.run = None;
                let inferred = self.inferred_triples(&classification);
                let result = json!({
                    "status": "done",
                    "classification": classification,
                    "inferred_triples": inferred.len(),
                });
                self.classified = Some(Classified {
                    classification,
                    inferred,
                });
                // Inferred and combined datasets depended on the old (absent) result.
                self.datasets
                    .retain(|(sel, _, _)| *sel == DatasetSelection::Asserted);
                Ok(result)
            }
            Progress::Stopped { code, steps } => {
                self.run = None;
                Err(wire(
                    &code,
                    format!("classification stopped after {steps} steps"),
                ))
            }
        }
    }

    /// Entailed triples: every subsumption (told and derived), equivalences,
    /// unsatisfiable classes as subclasses of `owl:Nothing`, and individuals'
    /// types closed under subsumption.
    fn inferred_triples(&mut self, c: &Classification) -> Vec<Triple> {
        let mut out = TripleSet::default();
        let n = |s: &str| NamedNode::new_unchecked(s);
        let mut sups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for s in &c.subsumptions {
            out.insert(Triple::new(
                NamedOrBlankNode::NamedNode(n(&s.sub)),
                rdfs::SUB_CLASS_OF.into_owned(),
                n(&s.sup),
            ));
            sups.entry(&s.sub).or_default().push(&s.sup);
        }
        for group in &c.equivalences {
            for a in group {
                for b in group {
                    if a != b {
                        out.insert(Triple::new(
                            NamedOrBlankNode::NamedNode(n(a)),
                            n(owl::EQUIVALENT_CLASS),
                            n(b),
                        ));
                    }
                }
            }
        }
        for u in &c.unsatisfiable {
            out.insert(Triple::new(
                NamedOrBlankNode::NamedNode(n(u)),
                rdfs::SUB_CLASS_OF.into_owned(),
                n(owl::NOTHING),
            ));
        }
        let types = self.compiled().individual_types.clone();
        for (individual, classes) in &types {
            for class in classes {
                out.insert(Triple::new(
                    NamedOrBlankNode::NamedNode(n(individual)),
                    rdf::TYPE.into_owned(),
                    n(class),
                ));
                for sup in sups.get(class.as_str()).into_iter().flatten() {
                    out.insert(Triple::new(
                        NamedOrBlankNode::NamedNode(n(individual)),
                        rdf::TYPE.into_owned(),
                        n(sup),
                    ));
                }
            }
        }
        out.into_vec()
    }

    fn query(
        &mut self,
        sparql: &str,
        selection: DatasetSelection,
        limit: Option<usize>,
    ) -> Result<Json, WireError> {
        let parsed = query::parse_checked(sparql).map_err(|e| wire(e.code(), e.to_string()))?;
        if selection != DatasetSelection::Asserted && self.classified.is_none() {
            return Err(wire(
                "not_classified",
                "the inferred dataset needs a classification of the current generation",
            ));
        }
        let named = query::uses_graph(&parsed);
        // A query without GRAPH can reuse a dataset that also has named graphs.
        let cached = self
            .datasets
            .iter()
            .position(|(sel, n, _)| *sel == selection && (*n || !named));
        let at = match cached {
            Some(i) => i,
            None => {
                self.compiled();
                let asserted = self.compiled.as_ref().map_or(&[][..], |c| &c.triples[..]);
                let inferred = self
                    .classified
                    .as_ref()
                    .map_or(&[][..], |c| &c.inferred[..]);
                let ds = Store::build(asserted, inferred, selection, named);
                self.datasets.push((selection, named, ds));
                self.datasets.len() - 1
            }
        };
        let out = query::evaluate(
            &parsed,
            &self.datasets[at].2,
            limit.unwrap_or(query::DEFAULT_ROW_LIMIT),
        )
        .map_err(|e| wire(e.code(), e.to_string()))?;
        serde_json::to_value(out).map_err(|e| wire("internal", e.to_string()))
    }

    fn propose(
        &mut self,
        path: &str,
        base_digest: &str,
        ops: Vec<PatchOp>,
    ) -> Result<Json, WireError> {
        let doc = self
            .docs
            .get(path)
            .ok_or_else(|| wire("not_found", format!("no document {path}")))?;
        let digest = doc.digest();
        if digest.to_string() != base_digest {
            return Err(wire(
                "stale_base",
                format!("{path} is {digest}, the patch was planned against {base_digest}"),
            ));
        }
        let mut patch = Patch::new(digest);
        patch.ops = ops;
        let applied = doc
            .apply(&patch)
            .map_err(|e| wire("patch_refused", e.to_string()))?;
        let new_text = applied.document.source();
        Ok(json!({
            "path": path,
            "base_digest": base_digest,
            "base_text": doc.source(),
            "new_text": new_text,
            "new_digest": applied.receipt.after.to_string(),
            "diff": unified_diff(path, doc.source(), new_text),
            "edits": applied.receipt.edits,
            "preserved_bytes": applied.receipt.preserved_bytes,
            "no_op": applied.receipt.edits.is_empty(),
        }))
    }
}

fn wire(code: &str, message: impl Into<String>) -> WireError {
    WireError {
        code: code.into(),
        message: message.into(),
    }
}

fn is_markdown(path: &str) -> bool {
    path.len() > 3 && path[path.len() - 3..].eq_ignore_ascii_case(".md")
}
