//! Turning model responses into guarded candidates.

use std::collections::BTreeSet;

use indexmap::IndexMap;
use ontomark_model::{DocId, Iri, Kind, Model, PredicateForm, Profile, Resolution, Severity};
use ontomark_parse::{Document, Patch, Span, Value, Wikilink};
use serde::{Deserialize, Serialize};

use crate::chunk::chunk_document;
use crate::contract::{key_specs, KeySpec, ModelRequest, ModelResponse, ValueKind, CONTRACT};
use crate::error::IngestError;
use crate::normalize::normalize_text;
use crate::similarity::AliasTier;
use crate::vocabulary::{Grounding, Suggestion, Term, Vocabulary};
use crate::Options;

/// Starting confidence of every candidate, from the retired RDF-to-Logseq
/// adapter. Ingest never raises it.
pub const CONFIDENCE: f32 = 0.5;

/// Starting grounding method of every candidate.
pub const METHOD: &str = "inferred";

/// A span in UTF-16 code units, as Obsidian's editor counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Utf16Span {
    /// First code unit.
    pub start: usize,
    /// One past the last code unit.
    pub end: usize,
}

/// Where in the note a value is supported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSpan {
    /// The chunk the quote was found in.
    pub chunk: usize,
    /// Byte span in the document source.
    pub span: Span,
    /// The same span in UTF-16 code units.
    pub utf16: Utf16Span,
    /// The source text of the span.
    pub quote: String,
}

/// A value the candidate patch writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateAssertion {
    /// The frontmatter key.
    pub key: String,
    /// The predicate it asserts.
    pub predicate: Iri,
    /// The value exactly as the patch writes it (`[[Note]]` for a name).
    pub written: String,
    /// The entity a name grounded to.
    pub grounded: Option<Iri>,
    /// The tier that grounded it.
    pub tier: Option<AliasTier>,
    /// The literal datatype, for literals.
    pub datatype: Option<Iri>,
    /// Every place the note supports it.
    pub evidence: Vec<EvidenceSpan>,
    /// [`CONFIDENCE`].
    pub confidence: f32,
    /// [`METHOD`].
    pub method: String,
}

/// Why a proposed value was not written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum WithholdReason {
    /// The key is not in the request.
    UnknownKey,
    /// The quote was not found in the chunk.
    EvidenceNotFound,
    /// The quote was found but does not mention the value: neither the
    /// proposed text nor any name of the grounded entity (for a name), nor
    /// the literal as proposed or as written (for a literal).
    EvidenceDoesNotMention,
    /// The name matched no entity.
    Ungrounded {
        /// Near misses, best first.
        suggestions: Vec<Suggestion>,
    },
    /// The name matched several entities.
    Ambiguous {
        /// The candidates.
        candidates: Vec<Iri>,
    },
    /// The name differs from an entity's only by a conflicting initial.
    InitialsConflict {
        /// That entity.
        with: Iri,
    },
    /// The grounded entity has the wrong kind for the predicate.
    KindMismatch {
        /// What the predicate form needs.
        expected: Kind,
        /// What grounded.
        found: Kind,
    },
    /// The value names the note's own entity.
    SelfReference,
    /// Neither the bare note name nor the vault path resolves back to the
    /// grounded entity alone, so no link can be written for it.
    Unlinkable {
        /// The link targets tried, in order.
        tried: Vec<String>,
    },
    /// Several differing values were proposed for a single-valued key in
    /// one acceptance; none is written.
    CompetingValues {
        /// The other values proposed, as they would have been written.
        others: Vec<String>,
    },
    /// The literal is not in the lexical space of its datatype.
    BadLiteral {
        /// The datatype.
        datatype: Iri,
        /// What was wrong.
        detail: String,
    },
    /// The key holds a single value that differs.
    ConflictingValue {
        /// The value already there.
        existing: String,
    },
    /// The key's existing value is not a scalar or list, so it cannot be
    /// extended safely.
    UnpatchableKey,
    /// The note already holds this value.
    AlreadyPresent,
}

/// A proposed value that was not written, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Withheld {
    /// The chunk it came from.
    pub chunk: usize,
    /// The key proposed.
    pub key: String,
    /// The value proposed, as the model wrote it.
    pub value: String,
    /// Why.
    pub reason: WithholdReason,
    /// Evidence, when it was found.
    pub evidence: Option<EvidenceSpan>,
}

/// A note about the acceptance that is not attached to one value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestDiagnostic {
    /// Severity.
    pub severity: Severity,
    /// Stable code.
    pub code: String,
    /// Message.
    pub message: String,
}

/// The result of accepting responses: at most one patch, bound to the
/// digest the request was planned against and already proven to apply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidates {
    /// The note.
    pub document: DocId,
    /// The digest (hex) the patch is bound to.
    pub digest: String,
    /// The patch, when anything was accepted.
    pub patch: Option<Patch>,
    /// What the patch writes.
    pub accepted: Vec<CandidateAssertion>,
    /// What was refused.
    pub withheld: Vec<Withheld>,
    /// Other notes.
    pub diagnostics: Vec<IngestDiagnostic>,
}

/// Stable diagnostic codes.
pub mod codes {
    /// A literal was trimmed, unquoted or had separators removed before it
    /// was accepted.
    pub const LITERAL_REPAIRED: &str = "literal_repaired";
    /// A response proposed nothing.
    pub const EMPTY_RESPONSE: &str = "empty_response";
}

enum Resolved {
    Name {
        iri: Iri,
        tier: AliasTier,
        written: String,
    },
    Literal {
        lexical: String,
        datatype: Iri,
    },
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn accept(
    profile: &dyn Profile,
    model: &Model,
    vocabulary: &Vocabulary,
    options: &Options,
    doc: &Document,
    request: &ModelRequest,
    responses: &[ModelResponse],
) -> Result<Candidates, IngestError> {
    if request.contract != CONTRACT {
        return Err(IngestError::ContractMismatch {
            expected: CONTRACT.to_owned(),
            found: request.contract.clone(),
        });
    }
    let digest = doc.digest();
    if request.note.digest != digest.hex() {
        return Err(IngestError::StaleRevision {
            expected: request.note.digest.clone(),
            found: digest.hex(),
        });
    }
    // The request is the host's copy of what `plan` produced. Everything in
    // it that drives a write is recomputed here and must agree, so a request
    // altered in transit or built by hand can neither name keys the profile
    // does not grant nor carry spans outside the note.
    let entity =
        model
            .entity_of(&request.note.document)
            .ok_or_else(|| IngestError::NotAnEntity {
                document: request.note.document.clone(),
            })?;
    if entity.iri != request.note.iri
        || entity.kind != request.note.kind
        || entity.label != request.note.label
    {
        return Err(IngestError::RequestMismatch { field: "note" });
    }
    if key_specs(profile, doc, entity.kind) != request.keys {
        return Err(IngestError::RequestMismatch { field: "keys" });
    }
    if chunk_document(doc, options.min_chunk_chars, options.max_chunk_chars) != request.chunks {
        return Err(IngestError::RequestMismatch { field: "chunks" });
    }
    for r in responses {
        if r.chunk >= request.chunks.len() {
            return Err(IngestError::UnknownChunk {
                chunk: r.chunk,
                count: request.chunks.len(),
            });
        }
    }
    let utf16 = doc.utf16_index();
    let subject = &request.note;
    // Frontmatter the parser could not read is never patched; every value
    // is withheld rather than the whole batch failing at the proof.
    let locked = doc.frontmatter().is_some_and(|f| !f.is_parsed());
    let mut diagnostics = Vec::new();
    let mut withheld = Vec::new();
    // key -> identity -> candidate, in first-seen order. A name's identity
    // is its grounded IRI, so two spellings of one entity merge and two
    // entities never do; a literal's identity is its written form.
    let mut accepted: IndexMap<String, IndexMap<String, CandidateAssertion>> = IndexMap::new();

    let mut ordered: Vec<&ModelResponse> = responses.iter().collect();
    ordered.sort_by_key(|r| r.chunk);

    for response in ordered {
        if response.assertions.is_empty() {
            diagnostics.push(IngestDiagnostic {
                severity: Severity::Info,
                code: codes::EMPTY_RESPONSE.to_owned(),
                message: format!("chunk {} proposed nothing", response.chunk),
            });
        }
        let chunk = &request.chunks[response.chunk];
        for a in &response.assertions {
            let mut refuse = |reason: WithholdReason, evidence: Option<EvidenceSpan>| {
                withheld.push(Withheld {
                    chunk: response.chunk,
                    key: a.key.clone(),
                    value: a.value.clone(),
                    reason,
                    evidence,
                });
            };
            let Some(spec) = request.keys.iter().find(|k| k.key == a.key) else {
                refuse(WithholdReason::UnknownKey, None);
                continue;
            };
            if locked {
                refuse(WithholdReason::UnpatchableKey, None);
                continue;
            }
            let evidence = locate(&chunk.text, &a.evidence).map(|(s, e)| {
                let span = Span::new(chunk.span.start + s, chunk.span.start + e);
                EvidenceSpan {
                    chunk: response.chunk,
                    span,
                    // Chunk spans were just verified against the note and
                    // `locate` returns char boundaries, so this holds.
                    utf16: utf16
                        .span(span)
                        .map(|(start, end)| Utf16Span { start, end })
                        .expect("evidence spans lie on char boundaries of the source"),
                    quote: span.slice(doc.source()).to_owned(),
                }
            });
            let Some(evidence) = evidence else {
                refuse(WithholdReason::EvidenceNotFound, None);
                continue;
            };
            let resolved = match &spec.value {
                ValueKind::Name => {
                    match vocabulary.ground(
                        profile,
                        model,
                        &a.value,
                        &options.lexical,
                        options.suggestions,
                    ) {
                        Grounding::Resolved { iri, tier } => {
                            if iri == subject.iri {
                                refuse(WithholdReason::SelfReference, Some(evidence));
                                continue;
                            }
                            let Some(term) = vocabulary.term(&iri) else {
                                refuse(
                                    WithholdReason::Ungrounded {
                                        suggestions: Vec::new(),
                                    },
                                    Some(evidence),
                                );
                                continue;
                            };
                            if let Some(expected) = expected_kind(spec.form, subject.kind) {
                                if !kind_fits(expected, term.kind) {
                                    refuse(
                                        WithholdReason::KindMismatch {
                                            expected,
                                            found: term.kind,
                                        },
                                        Some(evidence),
                                    );
                                    continue;
                                }
                            }
                            let target = match link_target(profile, model, term, &iri) {
                                Ok(t) => t,
                                Err(tried) => {
                                    refuse(WithholdReason::Unlinkable { tried }, Some(evidence));
                                    continue;
                                }
                            };
                            Resolved::Name {
                                iri,
                                tier,
                                written: format!("[[{target}]]"),
                            }
                        }
                        Grounding::Ambiguous { candidates } => {
                            refuse(WithholdReason::Ambiguous { candidates }, Some(evidence));
                            continue;
                        }
                        Grounding::InitialsConflict { with } => {
                            refuse(WithholdReason::InitialsConflict { with }, Some(evidence));
                            continue;
                        }
                        Grounding::Ungrounded { suggestions } => {
                            refuse(WithholdReason::Ungrounded { suggestions }, Some(evidence));
                            continue;
                        }
                    }
                }
                ValueKind::Literal { datatype } => match check_literal(datatype, &a.value) {
                    Ok(lexical) => {
                        if lexical != a.value {
                            diagnostics.push(IngestDiagnostic {
                                severity: Severity::Info,
                                code: codes::LITERAL_REPAIRED.to_owned(),
                                message: format!(
                                    "`{}`: {:?} accepted as {:?}",
                                    spec.key, a.value, lexical
                                ),
                            });
                        }
                        Resolved::Literal {
                            lexical,
                            datatype: datatype.clone(),
                        }
                    }
                    Err(detail) => {
                        refuse(
                            WithholdReason::BadLiteral {
                                datatype: datatype.clone(),
                                detail,
                            },
                            Some(evidence),
                        );
                        continue;
                    }
                },
            };
            // Located is not enough: the quote has to mention the value.
            if !mentions(&evidence.quote, &a.value, &resolved, vocabulary) {
                refuse(WithholdReason::EvidenceDoesNotMention, Some(evidence));
                continue;
            }
            // Against what the note already holds.
            let existing = doc.value(&spec.key);
            match existing_state(profile, model, spec, existing, &resolved) {
                Existing::Present => {
                    refuse(WithholdReason::AlreadyPresent, Some(evidence));
                    continue;
                }
                Existing::Conflict(e) => {
                    refuse(
                        WithholdReason::ConflictingValue { existing: e },
                        Some(evidence),
                    );
                    continue;
                }
                Existing::Unpatchable => {
                    refuse(WithholdReason::UnpatchableKey, Some(evidence));
                    continue;
                }
                Existing::Extendable => {}
            }
            let (identity, written, grounded, tier, datatype) = match resolved {
                Resolved::Name { iri, tier, written } => (
                    iri.as_str().to_owned(),
                    written,
                    Some(iri),
                    Some(tier),
                    None,
                ),
                Resolved::Literal { lexical, datatype } => {
                    (lexical.clone(), lexical, None, None, Some(datatype))
                }
            };
            accepted
                .entry(spec.key.clone())
                .or_default()
                .entry(identity)
                .and_modify(|c| c.evidence.push(evidence.clone()))
                .or_insert_with(|| CandidateAssertion {
                    key: spec.key.clone(),
                    predicate: spec.predicate.clone(),
                    written,
                    grounded,
                    tier,
                    datatype,
                    evidence: vec![evidence],
                    confidence: CONFIDENCE,
                    method: METHOD.to_owned(),
                });
        }
    }

    // One patch, keys in profile order.
    let mut patch = Patch::new(digest);
    let mut flat = Vec::new();
    let mut any = false;
    for spec in &request.keys {
        let Some(values) = accepted.get(&spec.key) else {
            continue;
        };
        let existing = doc.value(&spec.key).filter(|v| !matches!(v, Value::Null));
        // A single-valued key with several differing proposals: none can be
        // trusted over the others, so all are withheld.
        if spec.form == PredicateForm::DataAssertion
            && values.len() > 1
            && !matches!(existing, Some(Value::Seq(_)))
        {
            let all: Vec<&str> = values.values().map(|c| c.written.as_str()).collect();
            for c in values.values() {
                let others: Vec<String> = all
                    .iter()
                    .filter(|w| **w != c.written)
                    .map(|w| (*w).to_owned())
                    .collect();
                for e in &c.evidence {
                    withheld.push(Withheld {
                        chunk: e.chunk,
                        key: c.key.clone(),
                        value: c.written.clone(),
                        reason: WithholdReason::CompetingValues {
                            others: others.clone(),
                        },
                        evidence: Some(e.clone()),
                    });
                }
            }
            continue;
        }
        // Literals are written with their YAML type so Obsidian types the
        // property the same way the RDF does; names are strings.
        let new: Vec<Value> = values
            .values()
            .map(|c| match &c.datatype {
                Some(dt) => literal_value(dt, &c.written),
                None => Value::string(&c.written),
            })
            .collect();
        let value = match (existing, &spec.value) {
            (None, ValueKind::Literal { .. }) if new.len() == 1 => new[0].clone(),
            (None, _) => Value::Seq(new),
            (Some(Value::Seq(items)), _) => {
                let mut items = items.clone();
                items.extend(new);
                Value::Seq(items)
            }
            (Some(scalar), _) => {
                // Promote the scalar as it is, keeping its type.
                let mut items = vec![scalar.clone()];
                items.extend(new);
                Value::Seq(items)
            }
        };
        patch = patch.set(&spec.key, value);
        any = true;
        flat.extend(values.values().cloned());
    }
    let patch = if any {
        // Prove it applies before handing it out.
        doc.apply(&patch)?;
        Some(patch)
    } else {
        None
    };
    Ok(Candidates {
        document: subject.document.clone(),
        digest: digest.hex(),
        patch,
        accepted: flat,
        withheld,
        diagnostics,
    })
}

enum Existing {
    Extendable,
    Present,
    Conflict(String),
    Unpatchable,
}

fn existing_state(
    profile: &dyn Profile,
    model: &Model,
    spec: &KeySpec,
    existing: Option<&Value>,
    resolved: &Resolved,
) -> Existing {
    // A key present with no value holds nothing to conflict with.
    let Some(existing) = existing.filter(|v| !matches!(v, Value::Null)) else {
        return Existing::Extendable;
    };
    let items: Vec<String> = match existing {
        Value::Seq(items) => {
            if items.iter().any(|i| i.scalar_text().is_none()) {
                return Existing::Unpatchable;
            }
            existing.texts()
        }
        Value::Map(_) => return Existing::Unpatchable,
        _ => existing.texts(),
    };
    let present = items.iter().any(|t| match resolved {
        Resolved::Name { iri, .. } => {
            let target = Wikilink::parse(t).map_or_else(|| t.clone(), |w| w.target);
            matches!(
                model.resolve_with(profile, &target).state,
                Resolution::Resolved(ref r) if r == iri
            )
        }
        Resolved::Literal { lexical, .. } => t.trim() == lexical,
    });
    if present {
        return Existing::Present;
    }
    if matches!(resolved, Resolved::Literal { .. })
        && spec.form == PredicateForm::DataAssertion
        && !matches!(existing, Value::Seq(_))
    {
        return Existing::Conflict(items.into_iter().next().unwrap_or_default());
    }
    Existing::Extendable
}

/// The link target to write for a grounded entity: the bare note name when
/// the model resolves it back to that entity alone, else the vault path.
/// When neither does, no link is safe to write, and the targets tried are
/// returned.
fn link_target(
    profile: &dyn Profile,
    model: &Model,
    term: &Term,
    iri: &Iri,
) -> Result<String, Vec<String>> {
    let tried = [
        term.note_name().to_owned(),
        term.document.as_str().to_owned(),
    ];
    for t in &tried {
        if matches!(
            model.resolve_with(profile, t).state,
            Resolution::Resolved(ref r) if r == iri
        ) {
            return Ok(t.clone());
        }
    }
    Err(tried.to_vec())
}

/// Whether a located quote actually mentions the value it is offered for.
///
/// A name counts as mentioned when the quote contains the proposed text, or
/// any name of the grounded entity (label, aliases, note name), as a phrase
/// or as a whole word of at least two characters. A literal counts only as
/// a phrase, as proposed or as written. Everything is compared after
/// [`normalize_text`].
fn mentions(quote: &str, proposed: &str, resolved: &Resolved, vocabulary: &Vocabulary) -> bool {
    let q = normalize_text(quote);
    let words: BTreeSet<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let phrase = |s: &str| {
        let n = normalize_text(s);
        !n.is_empty() && q.contains(&n)
    };
    let word = |s: &str| {
        normalize_text(s)
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w.chars().count() >= 2 && words.contains(w))
    };
    match resolved {
        Resolved::Name { iri, .. } => {
            let mut names = vec![proposed.to_owned()];
            if let Some(t) = vocabulary.term(iri) {
                names.push(t.label.clone());
                names.push(t.note_name().to_owned());
                names.extend(t.aliases.iter().cloned());
            }
            names.iter().any(|n| phrase(n) || word(n))
        }
        Resolved::Literal { lexical, .. } => phrase(proposed) || phrase(lexical),
    }
}

/// The kind a name must have for `form` when the subject has `subject`.
fn expected_kind(form: PredicateForm, subject: Kind) -> Option<Kind> {
    match form {
        PredicateForm::SubClassOf
        | PredicateForm::EquivalentClass
        | PredicateForm::DisjointWith
        | PredicateForm::ClassAssertion => Some(Kind::Class),
        PredicateForm::Existential => match subject {
            Kind::Individual => Some(Kind::Individual),
            _ => Some(Kind::Class),
        },
        PredicateForm::ObjectAssertion | PredicateForm::SameAs => Some(Kind::Individual),
        PredicateForm::SubPropertyOf => Some(subject),
        PredicateForm::Skos(_) | PredicateForm::Annotation | PredicateForm::DataAssertion => None,
    }
}

fn kind_fits(expected: Kind, found: Kind) -> bool {
    match (expected, found) {
        (Kind::Property(_), Kind::Property(_)) => true,
        (e, f) => e == f,
    }
}

/// Find `needle` in `hay`: exactly, then with whitespace runs collapsed,
/// then also ignoring ASCII case. Returns byte offsets into `hay`.
fn locate(hay: &str, needle: &str) -> Option<(usize, usize)> {
    let needle_trim = needle.trim();
    if needle_trim.is_empty() {
        return None;
    }
    if let Some(i) = hay.find(needle_trim) {
        return Some((i, i + needle_trim.len()));
    }
    // Collapse whitespace in both, keeping a map from collapsed to original.
    let (chay, map) = collapse(hay);
    let (cneedle, _) = collapse(needle_trim);
    if cneedle.is_empty() {
        return None;
    }
    let found = chay
        .find(&cneedle)
        .or_else(|| find_ignore_ascii_case(&chay, &cneedle))?;
    let start = map[found];
    let end_c = found + cneedle.len();
    let end = map.get(end_c).copied().unwrap_or(hay.len());
    Some((start, end))
}

/// Whitespace runs become one space. `map[i]` is the byte offset in the
/// original of collapsed byte `i`; one extra entry maps the collapsed end to
/// the end of the last non-whitespace char, so a match reaching the end
/// never swallows trailing whitespace. A space maps to the start of its
/// run; an end never lands on one because needles are trimmed.
fn collapse(s: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(s.len());
    let mut map = Vec::with_capacity(s.len() + 1);
    let mut in_ws = false;
    let mut last_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            if !in_ws && !out.is_empty() {
                out.push(' ');
                map.push(i);
            }
            in_ws = true;
        } else {
            in_ws = false;
            let len = c.len_utf8();
            for _ in 0..len {
                map.push(i);
            }
            last_end = i + len;
            out.push(c);
        }
    }
    if out.ends_with(' ') {
        out.pop();
        map.pop();
    }
    map.push(last_end);
    (out, map)
}

fn find_ignore_ascii_case(hay: &str, needle: &str) -> Option<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.len() > h.len() {
        return None;
    }
    (0..=h.len() - n.len())
        .filter(|&i| hay.is_char_boundary(i))
        .find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// Drop thousands separators, but only when every comma groups digits by
/// three (`1,200`, `-12,345,678.5`). Any other comma is not a separator and
/// is left in place for the number parser to refuse, so `3,5` never becomes
/// `35`.
fn strip_thousands(v: &str) -> String {
    let (sign, rest) = v.strip_prefix('-').map_or(("", v), |r| ("-", r));
    let (int, frac) = rest
        .split_once('.')
        .map_or((rest, None), |(i, f)| (i, Some(f)));
    let digits = |g: &str| g.bytes().all(|b| b.is_ascii_digit());
    let mut groups = int.split(',');
    let grouped = int.contains(',')
        && groups
            .next()
            .is_some_and(|g| (1..=3).contains(&g.len()) && digits(g))
        && groups.all(|g| g.len() == 3 && digits(g));
    if !grouped {
        return v.to_owned();
    }
    let mut out = format!("{sign}{}", int.replace(',', ""));
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// The local name of an XSD datatype IRI (`integer`, `date`, ...).
fn xsd_local(datatype: &Iri) -> &str {
    datatype
        .as_str()
        .rsplit(['#', '/'])
        .next()
        .unwrap_or_default()
}

/// The YAML value to write for a checked literal: typed for booleans and
/// numbers, a string otherwise.
fn literal_value(datatype: &Iri, lexical: &str) -> Value {
    match xsd_local(datatype) {
        "boolean" => Value::Bool(lexical == "true"),
        "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
        | "positiveInteger" | "negativeInteger" | "nonPositiveInteger" => lexical
            .parse()
            .map_or_else(|_| Value::string(lexical), Value::Int),
        "decimal" | "float" | "double" => Value::Float(lexical.to_owned()),
        _ => Value::string(lexical),
    }
}

/// `true` when `v` has a C0 control character other than tab, LF and CR.
fn has_control(v: &str) -> bool {
    v.chars()
        .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
}

/// Check a literal against its datatype's lexical space, repairing what a
/// model commonly gets wrong: surrounding whitespace, wrapping quotes and
/// thousands separators in numbers. Returns the lexical form to write.
fn check_literal(datatype: &Iri, raw: &str) -> Result<String, String> {
    let mut v = raw.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            v = v[1..v.len() - 1].trim();
        }
    }
    if v.is_empty() {
        return Err("empty".into());
    }
    if has_control(v) {
        return Err(format!("{v:?} has a control character"));
    }
    let local = xsd_local(datatype);
    match local {
        "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
        | "positiveInteger" | "negativeInteger" | "nonPositiveInteger" => {
            let digits = strip_thousands(v);
            let n: i64 = digits
                .parse()
                .map_err(|_| format!("{v:?} is not an integer"))?;
            let ok = match local {
                "nonNegativeInteger" => n >= 0,
                "positiveInteger" => n > 0,
                "negativeInteger" => n < 0,
                "nonPositiveInteger" => n <= 0,
                "int" => i32::try_from(n).is_ok(),
                "short" => i16::try_from(n).is_ok(),
                "byte" => i8::try_from(n).is_ok(),
                _ => true,
            };
            if !ok {
                return Err(format!("{n} is outside {local}"));
            }
            Ok(n.to_string())
        }
        "decimal" | "float" | "double" => {
            let digits = strip_thousands(v);
            // xsd:decimal has no exponent form.
            if local == "decimal" && digits.contains(['e', 'E']) {
                return Err(format!("{v:?} is not a decimal"));
            }
            digits
                .parse::<f64>()
                .map_err(|_| format!("{v:?} is not a number"))
                .and_then(|f| {
                    if f.is_finite() {
                        Ok(digits)
                    } else {
                        Err(format!("{v:?} is not finite"))
                    }
                })
        }
        "boolean" => match v {
            "true" | "false" => Ok(v.to_owned()),
            _ => Err(format!("{v:?} is not true or false")),
        },
        "date" => {
            check_date(v)?;
            Ok(v.to_owned())
        }
        "dateTime" => {
            let (d, rest) = v
                .split_once('T')
                .ok_or_else(|| format!("{v:?} has no time part"))?;
            check_date(d)?;
            let hhmm = rest.as_bytes();
            if hhmm.len() < 5 || !hhmm[..2].iter().all(u8::is_ascii_digit) || hhmm[2] != b':' {
                return Err(format!("{v:?} has a malformed time"));
            }
            Ok(v.to_owned())
        }
        "anyURI" => {
            if v.contains(':') && !v.contains(char::is_whitespace) {
                Ok(v.to_owned())
            } else {
                Err(format!("{v:?} is not a URI"))
            }
        }
        _ => Ok(v.to_owned()),
    }
}

fn check_date(v: &str) -> Result<(), String> {
    let parts: Vec<&str> = v.splitn(3, '-').collect();
    let err = || format!("{v:?} is not a YYYY-MM-DD date");
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(err());
    }
    let y: i32 = parts[0].parse().map_err(|_| err())?;
    let m: u32 = parts[1].parse().map_err(|_| err())?;
    let d: u32 = parts[2].parse().map_err(|_| err())?;
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(err()),
    };
    if d == 0 || d > days {
        return Err(err());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xsd(local: &str) -> Iri {
        Iri::parse(format!("http://www.w3.org/2001/XMLSchema#{local}")).unwrap()
    }

    #[test]
    fn locate_exact_collapsed_and_case() {
        let hay = "Alpha  beta\n  gamma. Delta";
        assert_eq!(locate(hay, "beta"), Some((7, 11)));
        let (s, e) = locate(hay, "alpha beta gamma").unwrap();
        assert_eq!(&hay[s..e], "Alpha  beta\n  gamma");
        let (s, e) = locate(hay, "  Delta ").unwrap();
        assert_eq!(&hay[s..e], "Delta");
        assert_eq!(locate(hay, "omega"), None);
        assert_eq!(locate(hay, "   "), None);
    }

    #[test]
    fn locate_handles_multibyte() {
        let hay = "Dvořák wrote  the quartet";
        let (s, e) = locate(hay, "Dvořák wrote the").unwrap();
        assert_eq!(&hay[s..e], "Dvořák wrote  the");
    }

    #[test]
    fn locate_never_swallows_trailing_whitespace() {
        let hay = "Alpha  beta \n\t";
        let (s, e) = locate(hay, "alpha beta").unwrap();
        assert_eq!(&hay[s..e], "Alpha  beta");
        let (s, e) = locate("x  ", "X").unwrap();
        assert_eq!((s, e), (0, 1));
    }

    #[test]
    fn thousands_are_stripped_only_when_grouped() {
        assert_eq!(strip_thousands("1,200"), "1200");
        assert_eq!(strip_thousands("-12,345,678.50"), "-12345678.50");
        assert_eq!(strip_thousands("3,5"), "3,5");
        assert_eq!(strip_thousands("12,34"), "12,34");
        assert_eq!(strip_thousands("1,2000"), "1,2000");
        assert_eq!(strip_thousands(",200"), ",200");
        assert_eq!(strip_thousands("200"), "200");
        assert!(check_literal(&xsd("decimal"), "3,5").is_err());
        assert!(check_literal(&xsd("integer"), "12,34").is_err());
        assert_eq!(
            check_literal(&xsd("decimal"), "1,200.5"),
            Ok("1200.5".into())
        );
    }

    #[test]
    fn literals_repair_or_refuse() {
        assert_eq!(
            check_literal(&xsd("integer"), " \"1,000\" "),
            Ok("1000".into())
        );
        assert!(check_literal(&xsd("integer"), "ten").is_err());
        assert!(check_literal(&xsd("positiveInteger"), "0").is_err());
        assert_eq!(check_literal(&xsd("decimal"), "3.5"), Ok("3.5".into()));
        assert!(check_literal(&xsd("decimal"), "NaN").is_err());
        assert_eq!(check_literal(&xsd("boolean"), "'true'"), Ok("true".into()));
        assert!(check_literal(&xsd("boolean"), "yes").is_err());
        assert_eq!(
            check_literal(&xsd("date"), "2024-02-29"),
            Ok("2024-02-29".into())
        );
        assert!(check_literal(&xsd("date"), "2023-02-29").is_err());
        assert!(check_literal(&xsd("date"), "1 May 2024").is_err());
        assert_eq!(
            check_literal(&xsd("dateTime"), "2024-05-01T12:00:00Z"),
            Ok("2024-05-01T12:00:00Z".into())
        );
        assert!(check_literal(&xsd("anyURI"), "not a uri").is_err());
        assert_eq!(check_literal(&xsd("string"), "  text "), Ok("text".into()));
        assert!(check_literal(&xsd("string"), "\"\"").is_err());
        // Second review pass.
        assert!(check_literal(&xsd("date"), "+202-+5-01").is_err());
        assert!(check_literal(&xsd("decimal"), "1e3").is_err());
        assert_eq!(check_literal(&xsd("double"), "1e3"), Ok("1e3".into()));
        assert!(check_literal(&xsd("byte"), "200").is_err());
        assert!(check_literal(&xsd("short"), "40000").is_err());
        assert!(check_literal(&xsd("int"), "3000000000").is_err());
        assert_eq!(check_literal(&xsd("int"), "-5"), Ok("-5".into()));
        assert!(check_literal(&xsd("string"), "a\u{0}b").is_err());
        assert_eq!(check_literal(&xsd("string"), "a\tb"), Ok("a\tb".into()));
    }

    #[test]
    fn literals_are_written_typed() {
        assert_eq!(literal_value(&xsd("boolean"), "true"), Value::Bool(true));
        assert_eq!(literal_value(&xsd("integer"), "1200"), Value::Int(1200));
        assert_eq!(
            literal_value(&xsd("decimal"), "3.50"),
            Value::Float("3.50".into())
        );
        assert_eq!(
            literal_value(&xsd("date"), "2024-05-01"),
            Value::string("2024-05-01")
        );
    }

    #[test]
    fn kinds_expected_per_form() {
        assert_eq!(
            expected_kind(PredicateForm::SubClassOf, Kind::Class),
            Some(Kind::Class)
        );
        assert_eq!(
            expected_kind(PredicateForm::Existential, Kind::Individual),
            Some(Kind::Individual)
        );
        assert_eq!(expected_kind(PredicateForm::Annotation, Kind::Class), None);
    }
}
