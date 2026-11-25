//! The build's intermediate model: a [`Corpus`] of [`ClassRecord`]s projected
//! out of frontmatter pages.
//!
//! Every artefact `vault build` emits is a pure function of this model, which
//! is what makes the golden parity test meaningful: the Python pipeline's
//! `PageData` / `OntologyEntity` pair is reproduced here field for field, so a
//! divergence in output is a divergence in one clearly located projection.
//!
//! # Reference resolution
//!
//! A frontmatter relation is a list of wikilinks; the artefacts need IRIs. A
//! target is resolved in this order:
//!
//! 1. a page with that id, title or alias — its `resource` is the IRI;
//! 2. an entry in the vocabulary's `tail_iris` map — the 163 long-tail
//!    concepts whose IRI is genuinely not derivable from their label
//!    (`IDA*` → `urn:ngm:class:ida-star`);
//! 3. `namespace + slugify(target)`.
//!
//! Step 2 exists because the json-ld fences carried explicit IRIs and 0.14% of
//! them cannot be recomputed. Losing them would silently re-point those edges.

use std::collections::HashMap;

use crate::input::page::{Page, Vault};
use crate::input::slug::{ref_slug, slugify, target_slug};
use crate::input::vocabulary::Vocabulary;
use indexmap::IndexMap;

/// The twelve relation attributes in `pipeline/reason.py::RELATION_TYPES`
/// order: `(frontmatter key, camelCase artefact key)`.
///
/// This is the **frozen v1 artefact contract**, not a vocabulary derivative: a
/// consumer that already parses `relationships.hasPart` must keep working
/// across a vocabulary rename.
pub const RELATION_KEYS: &[(&str, &str)] = &[
    ("has-part", "hasPart"),
    ("requires", "requires"),
    ("enables", "enables"),
    ("depends-on", "dependsOn"),
    ("implements", "implements"),
    ("contrasts-with", "contrastsWith"),
    ("bridges-to", "bridgesTo"),
    ("uses", "uses"),
    ("supports", "supports"),
    ("standardized-by", "standardizedBy"),
    ("part-of", "partOf"),
    ("related-to", "relatedTo"),
];

/// The frontmatter key that carries the taxonomy (`rdfs:subClassOf`).
pub const IS_A_KEY: &str = "is-a";
/// The frontmatter key that carries `rdf:type` for individuals.
pub const INSTANCE_OF_KEY: &str = "instance-of";
/// The artefact key `disjoint-with` (ADR-2125) is carried under in
/// [`ClassRecord::relations`]. It is deliberately **not** one of the
/// [`RELATION_KEYS`]: disjointness is a logical axiom, not a graph edge, so the
/// page API, OKF bundle and graph tiers never render it. Only the Turtle
/// emitter (as `owl:disjointWith`) and `vault validate` read it.
pub const DISJOINT_WITH_JSON_KEY: &str = "disjointWith";

/// One conjunct of a page's `defines-as` definition (ADR-2124), its target
/// resolved like any other reference. See [`crate::input::definition`] for the
/// frontmatter shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conjunct {
    /// A named class.
    Named(Ref),
    /// `∃relation.filler`, `relation` being the frontmatter relation key.
    Some {
        /// The relation's frontmatter key, e.g. `has-part`.
        relation: String,
        /// The named filler class.
        filler: Ref,
    },
}

impl Conjunct {
    /// The class this conjunct names: the class itself, or the filler.
    #[must_use]
    pub fn target(&self) -> &Ref {
        match self {
            Self::Named(r) | Self::Some { filler: r, .. } => r,
        }
    }
}

/// An IRI plus the label the citing page used for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// The resolved IRI.
    pub iri: String,
    /// The display label from the wikilink.
    pub label: String,
}

impl Ref {
    /// The IRI's slug (last colon segment).
    #[must_use]
    pub fn slug(&self) -> String {
        ref_slug(&self.iri)
    }
}

/// Whether a record is an OWL class or a named individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    /// `owl:Class`.
    Class,
    /// `owl:NamedIndividual`.
    Individual,
}

impl EntityType {
    /// The artefact spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::Individual => "Individual",
        }
    }
}

/// One `supersedes` entry as the emitters need it (ADR-VG-010 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersessionFact {
    /// The replaced page or folded loser as an IRI: a wikilink resolved to the
    /// page's resource, or the IRI as written. `None` for a withdrawal entry,
    /// which names no page.
    pub target: Option<String>,
    /// `world-changed`, `corrected`, `merged`, `renamed` or `withdrawn`.
    pub reason: String,
}

/// One typed `sources` entry that may be published (ADR-VG-010 §5, §7): its
/// target is a public page's identity IRI or an external IRI, never a page the
/// build withholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFact {
    /// The local citation handle (`id`).
    pub id: String,
    /// The relation: `origin`, `episode`, `evidence`, `derived-from`,
    /// `standard` or `reference` (an absent `rel` is `reference`).
    pub rel: String,
    /// The target's IRI.
    pub target: String,
    /// The claim or publication date, as written.
    pub at: Option<String>,
    /// What inside the target is cited, as written.
    pub locator: Option<String>,
}

/// One `statements` entry (ADR-VG-010 §4): a time-qualified value of one
/// relation. It is open when it has no `valid_to` and closed when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementFact {
    /// The relation key as written (`requires`, `standardized-by`, …).
    pub key: String,
    /// The value: a wikilink resolved to its IRI, or an IRI as written.
    pub value: String,
    /// `valid-from`, when it is a strict UTC second-precision instant.
    pub valid_from: Option<String>,
    /// `valid-to`, when it is a strict UTC second-precision instant.
    pub valid_to: Option<String>,
    /// `reason`: `world-changed`, `corrected`, `merged`, `renamed` or `withdrawn`.
    pub reason: Option<String>,
    /// `replaced-by`, resolved to an IRI.
    pub replaced_by: Option<String>,
    /// `at`: when the close was recorded, as written.
    pub at: Option<String>,
    /// `by`: the canonical principal DID of whoever recorded it.
    pub by: Option<String>,
    /// `sources`: ids of the page's own `sources` entries.
    pub sources: Vec<String>,
}

/// What a page says under vocabulary version 3 that the emitters publish
/// (ADR-VG-010 §7, ADR-VG-011 §1). Absent under version 2, so a version 2 build
/// is byte-identical to the releases before it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct V3Facts {
    /// The page's `status`, as written.
    pub status: Option<String>,
    /// `valid-from`, when it is a strict UTC second-precision instant.
    pub valid_from: Option<String>,
    /// `valid-to`, when it is a strict UTC second-precision instant.
    pub valid_to: Option<String>,
    /// `superseded-by` links, resolved to IRIs.
    pub superseded_by: Vec<String>,
    /// `supersedes` entries.
    pub supersedes: Vec<SupersessionFact>,
    /// The canonical principal DID of `generated.by`, through the roster when
    /// there is one; `None` when `by` is not a person's key.
    pub principal: Option<String>,
    /// `generated.via`, the instrument (`agent:<name>/<version>` or
    /// `process:<name>/<version>`), as written.
    pub via: Option<String>,
    /// The page's `type`.
    pub type_name: Option<String>,
    /// Whether the page's type is a durable memory kind.
    pub memory: bool,
    /// `statements` entries (ADR-VG-010 §4), values resolved to IRIs.
    pub statements: Vec<StatementFact>,
    /// Typed `sources` that may be published, in the author's order. Filled by
    /// [`Corpus::attach_sources`]; empty until then.
    pub sources: Vec<SourceFact>,
    /// How many `sources` entries [`Corpus::attach_sources`] withheld: the
    /// target is not public, or the entry makes a `Brief` evidence or origin.
    /// Only the count is kept; nothing about the target.
    pub sources_withheld: usize,
}

impl V3Facts {
    /// `true` when current recall may return this page at `now` (Unix
    /// seconds): an `approved` durable memory kind whose `valid-from` is not
    /// after `now` (absent reads as started, ruling B5) and whose `valid-to` is
    /// not at or before `now` (ADR-VG-010 §6, ADR-VG-013 §1). It is the record
    /// form of `check::v3::state::is_recallable`, which reads the page.
    #[must_use]
    pub fn recallable_at(&self, now: i64) -> bool {
        use crate::input::time::parse_utc_seconds;
        self.memory
            && self.status.as_deref() == Some("approved")
            && self
                .valid_from
                .as_deref()
                .and_then(parse_utc_seconds)
                .is_none_or(|t| t <= now)
            && self
                .valid_to
                .as_deref()
                .and_then(parse_utc_seconds)
                .is_none_or(|t| t > now)
    }
}

/// One page projected into the ontology model.
#[derive(Debug, Clone)]
pub struct ClassRecord {
    /// The page id (vault-relative path without `.md`).
    pub page_id: String,
    /// The page's publish slug.
    pub slug: String,
    /// The page's title.
    pub title: String,
    /// `public: true`?
    pub public: bool,
    /// The page's identity IRI (`urn:visionflow:page:…` historically; the
    /// class IRI when no separate page IRI exists).
    pub page_iri: String,
    /// The class/individual IRI (`resource`).
    pub iri: String,
    /// `rdfs:label`.
    pub label: String,
    /// Class or individual.
    pub entity_type: EntityType,
    /// `domain`.
    pub domain: String,
    /// `definition`.
    pub definition: String,
    /// `maturity`.
    pub maturity: String,
    /// `quality`.
    pub quality: f64,
    /// `legacy-term-id` — the pre-urn identifier (`AI-0376`, `RB-0162`) the
    /// `WebVOWL` node attributes carry as `term_id`. 2,466 pages have one.
    pub legacy_term_id: String,
    /// Who generated the page, from the OKF `generated.by` stamp (an
    /// [`Actor`](crate::input::okf::Actor)), or
    /// `None` when the page carries no parseable stamp. Resolved to a `did:nostr`
    /// by the attribution roster at emit time; never guessed.
    /// Explicit source attestations; never synthesised from compiler identity.
    pub attestations: serde_json::Map<String, serde_json::Value>,
    /// Parsed content generator, when explicitly supplied.
    pub generated_by: Option<crate::input::okf::Actor>,
    /// Which keys human `verified` stamps cover (ADR-OM-012).
    pub certification: crate::input::okf::Certification,
    /// `is-a`.
    pub sub_class_of: Vec<Ref>,
    /// `instance-of`.
    pub instance_of: Vec<Ref>,
    /// The twelve typed relations, keyed by camelCase artefact key, in
    /// [`RELATION_KEYS`] order, then `disjoint-with` under
    /// [`DISJOINT_WITH_JSON_KEY`]; empty lists are absent.
    pub relations: IndexMap<&'static str, Vec<Ref>>,
    /// Curated outbound wikilinks.
    pub links: Vec<Ref>,
    /// The page body.
    pub body: String,
    /// Whether the page declared an ontology identity at all. A page without
    /// `resource` and without `type` is a plain note: it still appears in the
    /// search index but contributes no OWL.
    pub has_ontology: bool,
    /// The page's `defines-as` definition (ADR-2124): a conjunction that is
    /// *equivalent* to this class. Empty for a primitive class — every class
    /// but a curated few — and for a value `vault validate` refuses as
    /// `NON_EL_DEFINITION`, so a malformed definition never reaches the OWL.
    pub defines_as: Vec<Conjunct>,
    /// Version 3 facts; `None` unless the vocabulary is version 3.
    pub v3: Option<Box<V3Facts>>,
}

impl ClassRecord {
    /// The class slug used to key every artefact — the IRI's tail, **not** the
    /// page slug. 96 pages in the corpus differ between the two.
    #[must_use]
    pub fn class_slug(&self) -> String {
        ref_slug(&self.iri)
    }

    /// The refs under a camelCase relation key.
    #[must_use]
    pub fn relation(&self, json_key: &str) -> &[Ref] {
        self.relations.get(json_key).map_or(&[], Vec::as_slice)
    }
}

/// The resolved corpus: every record plus the indexes the build needs.
#[derive(Debug, Clone)]
pub struct Corpus {
    /// Records in page order.
    pub records: Vec<ClassRecord>,
    /// Class slug to index into [`Corpus::records`], first definition winning.
    pub by_class_slug: IndexMap<String, usize>,
}

/// Index from lowercased name (id, title, alias) to page position.
#[derive(Debug)]
pub(crate) struct NameIndex {
    by_name: HashMap<String, usize>,
}

impl NameIndex {
    fn build(pages: &[Page]) -> Self {
        Self::build_iter(pages.iter())
    }

    /// The same index over any sequence of pages (position = iteration order).
    pub(crate) fn build_iter<'a>(pages: impl Iterator<Item = &'a Page>) -> Self {
        let mut by_name = HashMap::new();
        for (i, page) in pages.enumerate() {
            let mut add = |k: String| {
                by_name.entry(k).or_insert(i);
            };
            add(page.id.to_lowercase());
            add(page.title().to_lowercase());
            // A slug-targeted wikilink (the canonical spelling for a
            // long-tail reference) must still connect if a page is later
            // authored for that concept.
            add(page.slug().to_lowercase());
            for a in page.frontmatter.strings("aliases") {
                add(a.to_lowercase());
            }
        }
        Self { by_name }
    }

    pub(crate) fn get(&self, name: &str) -> Option<usize> {
        self.by_name.get(&name.to_lowercase()).copied()
    }
}

/// Give every public record a unique publish slug.
///
/// The slug names each page's published files (`api/pages/<slug>.json`) and
/// is its search-index `id`, which is what the explorer fetches by. It is the
/// declared `slug`, else `slugify(title)` — and those two sources can collide:
/// `ML Experiment Tracking` declares `slug: experiment-tracking`, while
/// `Experiment Tracking` derives the same slug from its title. Left alone, the
/// second page's file silently overwrote the first's.
///
/// Among public records sharing a slug, one keeps it: the page that
/// **declares** it (the migration carried that slug over as the page's
/// published identity), else the one whose `resource` tail equals it, else the
/// first in page order. Every other claimant is re-keyed to its own `resource`
/// tail — unique, immutable, and already the IRI Loom and `VisionClaw` key on.
/// Private records are left alone: they publish nothing, and a public/private
/// clash is refused by [`crate::ir::projection::project`] rather than resolved by
/// renaming a public page.
fn disambiguate_public_slugs(records: &mut [ClassRecord], declares_slug: &[bool]) {
    let mut claims: IndexMap<String, Vec<usize>> = IndexMap::new();
    for (i, r) in records.iter().enumerate() {
        if r.public && !r.slug.is_empty() {
            claims.entry(r.slug.clone()).or_default().push(i);
        }
    }
    for (slug, claimants) in claims {
        if claimants.len() < 2 {
            continue;
        }
        let keeper = claimants
            .iter()
            .copied()
            .find(|&i| declares_slug[i])
            .or_else(|| {
                claimants
                    .iter()
                    .copied()
                    .find(|&i| ref_slug(&records[i].iri) == slug)
            })
            .unwrap_or(claimants[0]);
        for i in claimants {
            if i != keeper {
                let tail = ref_slug(&records[i].iri);
                if !tail.is_empty() {
                    records[i].slug = tail;
                }
            }
        }
    }
}

/// A page's `defines-as` conjuncts, resolved (ADR-2124). Empty when the key
/// is absent, unregistered, or not a well-formed EL definition — `vault
/// validate` reports the last as `NON_EL_DEFINITION`.
fn definition_of(
    page: &Page,
    vocab: &Vocabulary,
    resolve: &impl Fn(&str, &str) -> Ref,
) -> Vec<Conjunct> {
    use crate::input::definition::{self, DEFINES_AS_KEY};
    if !vocab.is_relation(DEFINES_AS_KEY) {
        return Vec::new();
    }
    let Some(Ok(conjuncts)) = page
        .frontmatter
        .get(DEFINES_AS_KEY)
        .map(|value| definition::parse(value, vocab))
    else {
        return Vec::new();
    };
    conjuncts
        .into_iter()
        .map(|c| match c {
            definition::Conjunct::Named(w) => Conjunct::Named(resolve(&w.target, w.label())),
            definition::Conjunct::Some { relation, filler } => Conjunct::Some {
                relation,
                filler: resolve(&filler.target, filler.label()),
            },
        })
        .collect()
}

/// Read a page's version 3 facts (see [`V3Facts`]).
fn v3_facts(
    page: &Page,
    vocab: &Vocabulary,
    roster: Option<&crate::build::attribution::Roster>,
    resolve: &dyn Fn(&str, &str) -> Ref,
) -> V3Facts {
    use crate::input::frontmatter::Wikilink;
    use crate::input::time::parse_utc_seconds;
    use serde_norway::Value as Yaml;

    let fm = &page.frontmatter;
    let instant = |key: &str| fm.text(key).filter(|s| parse_utc_seconds(s).is_some());
    let type_name = fm.text("type");
    let generated = fm.get("generated");
    let generated_text = |key: &str| {
        generated
            .and_then(Yaml::as_mapping)
            .and_then(|m| m.get(Yaml::String(key.into())))
            .and_then(Yaml::as_str)
            .map(|s| s.trim().to_owned())
    };
    let principal = generated
        .and_then(crate::input::okf::Stamp::from_yaml)
        .and_then(|stamp| stamp.by.principal_did())
        .map(|did| {
            roster
                .and_then(|r| r.resolve_principal(&did))
                .map_or(did, |p| p.canonical)
        });
    let supersedes = match fm.get("supersedes") {
        Some(Yaml::Sequence(items)) => items
            .iter()
            .filter_map(Yaml::as_mapping)
            .filter_map(|m| {
                let text = |k: &str| {
                    m.get(Yaml::String(k.into()))
                        .and_then(Yaml::as_str)
                        .map(|s| s.trim().to_owned())
                };
                let reason = text("reason")?;
                let target = text("resource").and_then(|r| {
                    Wikilink::parse(&r).map_or_else(
                        || crate::check::v3::temporal::is_iri(&r).then_some(r),
                        |link| Some(resolve(&link.target, link.label()).iri),
                    )
                });
                Some(SupersessionFact { target, reason })
            })
            .collect(),
        _ => Vec::new(),
    };
    V3Facts {
        status: fm.text("status"),
        valid_from: instant("valid-from"),
        valid_to: instant("valid-to"),
        superseded_by: fm
            .wikilinks("superseded-by")
            .into_iter()
            .map(|w| resolve(&w.target, w.label()).iri)
            .collect(),
        supersedes,
        principal,
        via: generated_text("via"),
        memory: type_name
            .as_deref()
            .is_some_and(|t| vocab.working_type(t).memory),
        type_name,
        statements: statements_of(fm, roster, resolve),
        sources: Vec::new(),
        sources_withheld: 0,
    }
}

/// The page's `statements`, in the author's order. An entry with no usable
/// `key` or `value` is skipped (`INVALID_STATEMENT` reports it); the value is
/// resolved like any relation value.
fn statements_of(
    fm: &crate::input::frontmatter::Frontmatter,
    roster: Option<&crate::build::attribution::Roster>,
    resolve: &dyn Fn(&str, &str) -> Ref,
) -> Vec<StatementFact> {
    use crate::input::frontmatter::Wikilink;
    use crate::input::time::parse_utc_seconds;
    use serde_norway::Value as Yaml;

    let Some(Yaml::Sequence(items)) = fm.get("statements") else {
        return Vec::new();
    };
    let iri_of = |text: &str| -> Option<String> {
        Wikilink::parse(text).map_or_else(
            || is_safe_iri(text).then(|| text.to_owned()),
            |link| Some(resolve(&link.target, link.label()).iri),
        )
    };
    items
        .iter()
        .filter_map(Yaml::as_mapping)
        .filter_map(|m| {
            let text = |k: &str| {
                m.get(Yaml::String(k.into()))
                    .and_then(Yaml::as_str)
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            };
            let instant = |k: &str| text(k).filter(|s| parse_utc_seconds(s).is_some());
            let key = text("key")?;
            let value = iri_of(&text("value")?)?;
            let by = text("by")
                .and_then(|b| b.parse::<crate::input::okf::Actor>().ok())
                .and_then(|a| a.principal_did())
                .map(|did| {
                    roster
                        .and_then(|r| r.resolve_principal(&did))
                        .map_or(did, |p| p.canonical)
                });
            let sources = match m.get(Yaml::String("sources".into())) {
                Some(Yaml::Sequence(ids)) => ids
                    .iter()
                    .filter_map(Yaml::as_str)
                    .map(|s| s.trim().to_owned())
                    .collect(),
                _ => Vec::new(),
            };
            Some(StatementFact {
                key,
                value,
                valid_from: instant("valid-from"),
                valid_to: instant("valid-to"),
                reason: text("reason"),
                replaced_by: text("replaced-by").and_then(|r| iri_of(&r)),
                at: text("at"),
                by,
                sources,
            })
        })
        .collect()
}

/// An IRI that can be written between `<` and `>` in Turtle unescaped.
fn is_safe_iri(s: &str) -> bool {
    crate::check::v3::temporal::is_iri(s)
}

impl Corpus {
    /// Project a loaded vault through its vocabulary.
    #[must_use]
    pub fn build(vault: &Vault, vocab: &Vocabulary) -> Self {
        Self::build_with(vault, vocab, None)
    }

    /// [`Corpus::build`] reading certification against an attribution roster,
    /// so a `verified` stamp by a `did:nostr:` principal of `kind: human`
    /// certifies (vocabulary 3, where the old `human:` spelling does not). `None` is [`Corpus::build`].
    #[must_use]
    pub fn build_with(
        vault: &Vault,
        vocab: &Vocabulary,
        roster: Option<&crate::build::attribution::Roster>,
    ) -> Self {
        let names = NameIndex::build(&vault.pages);
        let resources: Vec<String> = vault
            .pages
            .iter()
            .map(|p| p.resource(&vocab.namespace))
            .collect();

        let resolve = |target: &str, label: &str| -> Ref {
            let iri = names
                .get(target)
                .map(|i| resources[i].clone())
                .or_else(|| vocab.tail_iris.get(label).cloned())
                .or_else(|| vocab.tail_iris.get(target).cloned())
                .unwrap_or_else(|| format!("{}{}", vocab.namespace, slugify(target)));
            Ref {
                iri,
                label: label.to_owned(),
            }
        };

        let links_of = |page: &Page, key: &str| -> Vec<Ref> {
            page.frontmatter
                .wikilinks(key)
                .into_iter()
                .map(|w| resolve(&w.target, w.label()))
                .collect()
        };

        let mut records = Vec::with_capacity(vault.pages.len());
        let mut declares_slug = Vec::with_capacity(vault.pages.len());
        for (i, page) in vault.pages.iter().enumerate() {
            declares_slug.push(page.frontmatter.has("slug"));
            let fm = &page.frontmatter;
            let title = page.title();
            let mut relations: IndexMap<&'static str, Vec<Ref>> = IndexMap::new();
            for (fm_key, json_key) in RELATION_KEYS {
                let refs = links_of(page, fm_key);
                if !refs.is_empty() {
                    relations.insert(json_key, refs);
                }
            }
            let disjoint = links_of(page, crate::input::consistency::DISJOINT_WITH_KEY);
            if !disjoint.is_empty() {
                relations.insert(DISJOINT_WITH_JSON_KEY, disjoint);
            }
            let entity_type = if fm.text("type").as_deref() == Some("Individual") {
                EntityType::Individual
            } else {
                EntityType::Class
            };
            records.push(ClassRecord {
                page_id: page.id.clone(),
                slug: page.slug(),
                label: fm
                    .text("label")
                    .filter(|l| !l.trim().is_empty())
                    .unwrap_or_else(|| title.clone()),
                title,
                public: crate::check::v3::state::is_public_under(page, vault.kind, vocab),
                page_iri: fm
                    .text("page_resource")
                    .unwrap_or_else(|| resources[i].clone()),
                iri: resources[i].clone(),
                entity_type,
                domain: fm.text("domain").unwrap_or_default(),
                // `definition_placement: leading-paragraph` (Q4): the fence
                // definition is prose in the body, not a frontmatter key. The
                // frontmatter form is still honoured for a vocabulary that
                // declares `frontmatter`, so this side stays data-driven too.
                definition: fm
                    .text("definition")
                    .filter(|d| !d.trim().is_empty())
                    .unwrap_or_else(|| page.leading_paragraph()),
                maturity: fm
                    .text("maturity")
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| "draft".to_owned()),
                quality: fm.number("quality").unwrap_or(0.0),
                legacy_term_id: fm.text("legacy-term-id").unwrap_or_default(),
                attestations: ["generated", "verified"]
                    .into_iter()
                    .filter_map(|key| {
                        fm.get(key)
                            .and_then(|value| serde_json::to_value(value).ok())
                            .map(|value| (key.to_owned(), value))
                    })
                    .collect(),
                generated_by: fm
                    .get("generated")
                    .and_then(crate::input::okf::Stamp::from_yaml)
                    .map(|st| st.by),
                certification: crate::input::okf::Certification::for_page_under(
                    page, vocab, roster,
                ),
                v3: vocab
                    .is_v3()
                    .then(|| Box::new(v3_facts(page, vocab, roster, &resolve))),
                sub_class_of: links_of(page, IS_A_KEY),
                instance_of: links_of(page, INSTANCE_OF_KEY),
                relations,
                // Curated links resolve **slug-verbatim**, not through the
                // name index. The backlink graph is keyed on the link's own
                // IRI tail, and 96 pages have a class slug that differs from
                // their page slug — resolving through the target page would
                // silently re-key every backlink list in the bundle.
                links: page
                    .outbound_links()
                    .into_iter()
                    .map(|w| Ref {
                        iri: format!("{}{}", vocab.namespace, target_slug(&w.target)),
                        label: w.label().to_owned(),
                    })
                    .collect(),
                body: page.body.clone(),
                has_ontology: fm.has("resource") || fm.has("type"),
                defines_as: definition_of(page, vocab, &resolve),
            });
        }

        disambiguate_public_slugs(&mut records, &declares_slug);

        let mut by_class_slug = IndexMap::new();
        for (i, r) in records.iter().enumerate() {
            if r.has_ontology && !r.iri.is_empty() {
                by_class_slug.entry(r.class_slug()).or_insert(i);
            }
        }

        Self {
            records,
            by_class_slug,
        }
    }

    /// Read each version 3 record's typed `sources` from `vault` (the vault
    /// this corpus was built from, as authored: not the redacted copy) and keep
    /// the entries that may be published (ADR-VG-010 §5, §7).
    ///
    /// A target is judged by `links`, the rule that redacts links, so a source
    /// and a link never disagree about which page is private. An entry is kept
    /// when it names a public page (by wikilink, `resource` or `page_resource`)
    /// or an external IRI. It is **withheld** and only counted when it names a
    /// page that is not public, or when it makes a `Brief` the `origin` or
    /// `evidence` of a claim (`BRIEF_AS_EVIDENCE` is a validation error, and
    /// emission never turns a brief into evidence). Free text, an id-only
    /// entry, a dangling wikilink, an unknown `rel` and a `urn:visionflow:page:`
    /// IRI no vault holds are skipped without a count: there is nothing to
    /// publish and nothing to leak.
    ///
    /// Only public records are read; other records keep no sources. A record
    /// with no version 3 facts is untouched, so a version 2 build is unchanged.
    pub fn attach_sources(&mut self, vault: &Vault, links: &crate::build::redact::PrivateLinks) {
        use crate::build::redact::SourceTarget;
        use serde_norway::Value as Yaml;

        const RELS: [&str; 6] = [
            "origin",
            "episode",
            "evidence",
            "derived-from",
            "standard",
            "reference",
        ];
        for (record, page) in self.records.iter_mut().zip(&vault.pages) {
            let (true, Some(facts)) = (record.public, record.v3.as_deref_mut()) else {
                continue;
            };
            let Some(Yaml::Sequence(items)) = page.frontmatter.get("sources") else {
                continue;
            };
            facts.sources.clear();
            facts.sources_withheld = 0;
            for item in items.iter().filter_map(Yaml::as_mapping) {
                let text = |k: &str| {
                    item.get(Yaml::String(k.into()))
                        .and_then(Yaml::as_str)
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                };
                let (Some(id), Some(resource)) = (text("id"), text("resource")) else {
                    continue;
                };
                let rel = text("rel").unwrap_or_else(|| "reference".to_owned());
                if !RELS.contains(&rel.as_str()) {
                    continue;
                }
                let evidential = matches!(rel.as_str(), "origin" | "evidence");
                let target = match links.source_target(vault.kind, &resource) {
                    SourceTarget::Private => {
                        facts.sources_withheld += 1;
                        continue;
                    }
                    SourceTarget::Public { iri, type_name } => {
                        if evidential && type_name.as_deref() == Some("Brief") {
                            facts.sources_withheld += 1;
                            continue;
                        }
                        iri
                    }
                    SourceTarget::Unresolved => {
                        if crate::input::frontmatter::Wikilink::parse(&resource).is_some()
                            || resource.starts_with("urn:visionflow:page:")
                            || !is_safe_iri(&resource)
                        {
                            continue;
                        }
                        resource
                    }
                };
                facts.sources.push(SourceFact {
                    id,
                    rel,
                    target,
                    at: text("at"),
                    locator: text("locator"),
                });
            }
        }
    }

    /// The records with `public: true`, in page order.
    pub fn public(&self) -> impl Iterator<Item = &ClassRecord> {
        self.records.iter().filter(|r| r.public)
    }

    /// The public records that are OWL classes with a non-empty IRI — the set
    /// `scaffold-index.json` and the class count are computed over.
    pub fn public_classes(&self) -> impl Iterator<Item = &ClassRecord> {
        self.records.iter().filter(|r| {
            r.public && r.has_ontology && !r.iri.is_empty() && r.entity_type == EntityType::Class
        })
    }

    /// Look a record up by class slug (first definition wins).
    #[must_use]
    pub fn by_slug(&self, slug: &str) -> Option<&ClassRecord> {
        self.by_class_slug.get(slug).map(|&i| &self.records[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::page::VaultKind;

    #[test]
    fn a_declared_slug_keeps_its_url_and_the_derived_claimant_moves_to_its_resource() {
        let vault = Vault {
            root: std::path::PathBuf::new(),
            kind: VaultKind::Knowledge,
            journals: Vec::new(),
            skipped: Vec::new(),
            pages: [
                (
                    "Experiment Tracking",
                    "---\ntype: Class\npublic: true\nresource: urn:ngm:class:empirical-experimental-design-tracking\n---\n",
                ),
                (
                    "ML Experiment Tracking",
                    "---\ntype: Class\npublic: true\nslug: experiment-tracking\nresource: urn:ngm:class:experiment-tracking\n---\n",
                ),
            ]
            .into_iter()
            .map(|(id, text)| {
                Page::parse(format!("/v/{id}.md"), format!("{id}.md"), id, text)
                    .unwrap()
            })
            .collect(),
        };
        let vocab =
            Vocabulary::from_yaml_str("version: 1\nnamespace: \"urn:ngm:class:\"\n").unwrap();
        let corpus = Corpus::build(&vault, &vocab);
        let slug_of = |id: &str| {
            corpus
                .records
                .iter()
                .find(|r| r.page_id == id)
                .unwrap()
                .slug
                .clone()
        };
        assert_eq!(slug_of("ML Experiment Tracking"), "experiment-tracking");
        assert_eq!(
            slug_of("Experiment Tracking"),
            "empirical-experimental-design-tracking"
        );
    }

    fn vocab() -> Vocabulary {
        Vocabulary::from_yaml_str(
            r#"
version: 1
namespace: "urn:ngm:class:"
relations:
  is-a:      { owl: "rdfs:subClassOf" }
  requires:  { owl: "vc:requires" }
  has-part:  { owl: "vc:hasPart" }
scalars:
  domain: { type: text }
tail_iris:
  "IDA*": "urn:ngm:class:ida-star"
"#,
        )
        .unwrap()
    }

    fn page(id: &str, fm: &str, body: &str) -> Page {
        Page::parse(
            format!("/v/{id}.md"),
            format!("{id}.md"),
            id,
            &format!("---\n{fm}---\n{body}"),
        )
        .unwrap()
    }

    fn corpus(pages: Vec<Page>) -> Corpus {
        let vault = Vault {
            root: std::path::PathBuf::new(),
            kind: VaultKind::Knowledge,
            journals: Vec::new(),
            skipped: Vec::new(),
            pages,
        };
        Corpus::build(&vault, &vocab())
    }

    fn v3_vocab() -> Vocabulary {
        Vocabulary::from_yaml_str(
            "version: 3\nnamespace: \"urn:ngm:class:\"\nworking_types:\n  Brief: {}\n  Episode: { memory: true }\n  Note: {}\nrelations:\n  is-a: { owl: \"rdfs:subClassOf\" }\n",
        )
        .unwrap()
    }

    fn vault_of(kind: VaultKind, pages: Vec<Page>) -> Vault {
        Vault {
            root: std::path::PathBuf::new(),
            kind,
            journals: Vec::new(),
            skipped: Vec::new(),
            pages,
        }
    }

    /// A public knowledge page citing every kind of target.
    fn sources_fixture() -> (Vault, Vault) {
        let knowledge = vault_of(
            VaultKind::Knowledge,
            vec![page(
                "Cites",
                "type: Class\npublic: true\nresource: urn:ngm:class:cites\nsources:\n\
                 - { id: a, resource: '[[Public Ledger]]', rel: origin }\n\
                 - { id: b, resource: '[[Secret Ledger]]', rel: origin }\n\
                 - { id: c, resource: '[[Public Brief]]', rel: evidence }\n\
                 - { id: d, resource: '[[Public Brief]]', rel: derived-from, locator: 'sha256-v1:ab' }\n\
                 - { id: e, resource: 'https://example.org/spec', rel: standard, at: '2022-10-25' }\n\
                 - { id: f, resource: 'urn:visionflow:page:secret1', rel: reference }\n\
                 - { id: g, resource: free text here, rel: reference }\n\
                 - { id: origin }\n\
                 - { id: h, resource: '[[No Such Page]]', rel: origin }\n\
                 - { id: i, resource: 'https://example.org/plain' }\n",
                "",
            )],
        );
        let working = vault_of(
            VaultKind::Working,
            vec![
                page(
                    "Public Ledger",
                    "type: Episode\nvisibility: public\npage_resource: urn:visionflow:page:pub1\n",
                    "",
                ),
                page(
                    "Secret Ledger",
                    "type: Episode\nvisibility: shared\npage_resource: urn:visionflow:page:secret1\n",
                    "",
                ),
                page(
                    "Public Brief",
                    "type: Brief\nvisibility: public\npage_resource: urn:visionflow:page:brief1\n",
                    "",
                ),
            ],
        );
        (knowledge, working)
    }

    #[test]
    fn typed_sources_keep_public_and_external_targets_and_withhold_the_rest() {
        let (knowledge, working) = sources_fixture();
        let vocab = v3_vocab();
        let links =
            crate::build::redact::PrivateLinks::new_with(&[&knowledge, &working], Some(&vocab));
        let mut c = Corpus::build_with(&knowledge, &vocab, None);
        c.attach_sources(&knowledge, &links);
        let facts = c.records[0].v3.as_deref().unwrap();
        let got: Vec<(&str, &str, &str)> = facts
            .sources
            .iter()
            .map(|s| (s.id.as_str(), s.rel.as_str(), s.target.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a", "origin", "urn:visionflow:page:pub1"),
                ("d", "derived-from", "urn:visionflow:page:brief1"),
                ("e", "standard", "https://example.org/spec"),
                ("i", "reference", "https://example.org/plain"),
            ]
        );
        // b and f name a non-public page (by wikilink and by page_resource); c makes a Brief evidence.
        assert_eq!(facts.sources_withheld, 3);
        assert_eq!(facts.sources[1].locator.as_deref(), Some("sha256-v1:ab"));
        assert_eq!(facts.sources[2].at.as_deref(), Some("2022-10-25"));
    }

    #[test]
    fn sources_are_read_only_under_version_3_and_only_for_public_records() {
        let (knowledge, working) = sources_fixture();
        let links = crate::build::redact::PrivateLinks::new_with(&[&knowledge, &working], None);
        let mut v2 = Corpus::build_with(&knowledge, &vocab(), None);
        v2.attach_sources(&knowledge, &links);
        assert!(v2.records[0].v3.is_none());

        let private = vault_of(
            VaultKind::Knowledge,
            vec![page(
                "Hidden",
                "type: Class\nresource: urn:ngm:class:hidden\nsources:\n- { id: a, resource: 'https://example.org/x', rel: origin }\n",
                "",
            )],
        );
        let vocab = v3_vocab();
        let links = crate::build::redact::PrivateLinks::new_with(&[&private], Some(&vocab));
        let mut c = Corpus::build_with(&private, &vocab, None);
        c.attach_sources(&private, &links);
        assert!(c.records[0].v3.as_deref().unwrap().sources.is_empty());
    }

    #[test]
    fn resolves_a_ref_through_the_target_pages_resource() {
        let c = corpus(vec![
            page(
                "Apple Vision Pro",
                "type: Class\npublic: true\nresource: urn:ngm:class:apple-inc-technology-corporation-vision-pro\n",
                "",
            ),
            page("Headset", "type: Class\npublic: true\nis-a: [\"[[Apple Vision Pro]]\"]\n", ""),
        ]);
        let r = &c.records[1].sub_class_of[0];
        assert_eq!(
            r.iri,
            "urn:ngm:class:apple-inc-technology-corporation-vision-pro"
        );
        assert_eq!(r.label, "Apple Vision Pro");
    }

    #[test]
    fn resolves_through_an_alias() {
        let c = corpus(vec![
            page(
                "Localisation",
                "type: Class\npublic: true\naliases: [Localization]\n",
                "",
            ),
            page(
                "LiDAR",
                "type: Class\npublic: true\nrequires: [\"[[Localization]]\"]\n",
                "",
            ),
        ]);
        assert_eq!(
            c.records[1].relation("requires")[0].iri,
            "urn:ngm:class:localisation"
        );
    }

    #[test]
    fn falls_back_to_a_tail_iri_then_to_slugification() {
        let c = corpus(vec![page(
            "Planner",
            "type: Class\npublic: true\nuses: []\nrequires: [\"[[IDA*]]\", \"[[Plain Concept]]\"]\n",
            "",
        )]);
        let refs = c.records[0].relation("requires");
        assert_eq!(refs[0].iri, "urn:ngm:class:ida-star");
        assert_eq!(refs[1].iri, "urn:ngm:class:plain-concept");
    }

    #[test]
    fn curated_links_keep_their_own_slug() {
        let c = corpus(vec![
            page(
                "Animation",
                "type: Class\npublic: true\nresource: urn:ngm:class:animation\nslug: 3-d-animation\n",
                "",
            ),
            page(
                "Viewer",
                "type: Class\npublic: true\nlinks: [\"[[3-d-animation|3D Animation]]\"]\n",
                "",
            ),
        ]);
        // Not `urn:ngm:class:animation`: the link carries its own identity.
        assert_eq!(c.records[1].links[0].iri, "urn:ngm:class:3-d-animation");
        assert_eq!(c.records[1].links[0].label, "3D Animation");
    }

    #[test]
    fn a_wikilink_alias_becomes_the_ref_label() {
        let c = corpus(vec![
            page("Localisation", "type: Class\npublic: true\n", ""),
            page(
                "LiDAR",
                "type: Class\npublic: true\nrequires: [\"[[Localisation|Localization]]\"]\n",
                "",
            ),
        ]);
        let r = &c.records[1].relation("requires")[0];
        assert_eq!(r.iri, "urn:ngm:class:localisation");
        assert_eq!(r.label, "Localization");
    }

    #[test]
    fn relations_keep_the_frozen_artefact_order() {
        let c = corpus(vec![page(
            "X",
            "type: Class\npublic: true\nrequires: [\"[[A]]\"]\nhas-part: [\"[[B]]\"]\n",
            "",
        )]);
        let keys: Vec<&str> = c.records[0].relations.keys().copied().collect();
        assert_eq!(keys, vec!["hasPart", "requires"]);
    }

    #[test]
    fn the_class_slug_comes_from_the_iri_not_the_page_slug() {
        let c = corpus(vec![page(
            "Bitcoin Cash",
            "type: Class\npublic: true\nslug: bitcoin-cash\nresource: urn:ngm:class:bitcoin-proof-of-work-protocol-cash\n",
            "",
        )]);
        assert_eq!(c.records[0].slug, "bitcoin-cash");
        assert_eq!(
            c.records[0].class_slug(),
            "bitcoin-proof-of-work-protocol-cash"
        );
        assert!(c.by_slug("bitcoin-proof-of-work-protocol-cash").is_some());
    }

    #[test]
    fn private_and_non_ontology_pages_are_excluded_from_the_class_set() {
        let c = corpus(vec![
            page("Pub", "type: Class\npublic: true\n", ""),
            page("Priv", "type: Class\npublic: false\n", ""),
            page("Note", "public: true\n", "just prose"),
            page("Ind", "type: Individual\npublic: true\n", ""),
        ]);
        let slugs: Vec<String> = c
            .public_classes()
            .map(super::ClassRecord::class_slug)
            .collect();
        assert_eq!(slugs, vec!["pub".to_owned()]);
        assert_eq!(c.public().count(), 3);
    }

    #[test]
    fn maturity_defaults_to_draft_and_quality_to_zero() {
        let c = corpus(vec![page("X", "type: Class\npublic: true\n", "")]);
        assert_eq!(c.records[0].maturity, "draft");
        assert!((c.records[0].quality - 0.0).abs() < f64::EPSILON);
    }
}
