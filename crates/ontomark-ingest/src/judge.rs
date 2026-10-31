//! Optional, host-supplied judgement hooks (the "Jev option").
//!
//! This crate performs no I/O, so it never asks a model anything. It exposes
//! three narrow hooks that a host may fill with a typed-decision service and
//! a deterministic default for each that does nothing:
//!
//! | Hook | Question put to the host | Default |
//! |---|---|---|
//! | [`FactJudge`] | does this quote entail this accepted fact? | [`NoJudge`] |
//! | [`CandidateSelector`] | which host-generated candidate, if any, does the text state? | [`NoSelector`] |
//! | [`FieldSuggester`] | which vocabulary term, if any, fills this key? | [`NoSuggester`] |
//!
//! Every hook is **advisory and monotone**:
//!
//! * A judge or selector can only *remove* values from what the plain path
//!   would have produced. [`Ingest::accept_judged`] re-runs the ordinary
//!   [`Ingest::accept`] on the surviving proposals, so quote grounding, name
//!   grounding, kind checks, conflict checks, the digest guard and the proof
//!   that the patch applies all still run, and it falls back to the plain
//!   result if the re-run would write anything the plain run did not.
//! * A suggester produces *proposals*, never a patch. Each is validated
//!   against the offered keys and the existing vocabulary; nothing is minted
//!   and reserved keys (such as `public`) are never offered.
//! * Any hook may report [`Unavailable`] (flag off, no endpoint, timeout,
//!   malformed answer, low confidence). The result is then exactly what the
//!   call without a hook returns.
//!
//! ```
//! use ontomark_ingest::{FactJudge, NoJudge};
//! // `NoJudge` is always unavailable, so `accept_judged(.., &NoJudge)`
//! // equals `accept(..)`.
//! let _judge: &dyn FactJudge = &NoJudge;
//! ```

use indexmap::IndexMap;
use ontomark_model::{DocId, Iri, Kind};
use ontomark_parse::Document;

use crate::accept::{expected_kind, kind_fits, link_target};
use crate::contract::{ModelRequest, ModelResponse, ValueKind};
use crate::error::IngestError;
use crate::normalize::{normalize_label, normalize_text};
use crate::vocabulary::Term;
use crate::{Candidates, Ingest, KeySpec, Withheld, WithholdReason};

/// A hook could not give an answer worth using. The reason is a stable
/// log code (`flag-off`, `no-endpoint`, `timeout`, `http-<code>`,
/// `malformed`, `low-confidence`); it never carries request content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    /// The log code.
    pub reason: String,
}

impl Unavailable {
    /// Build one from a log code.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// What happened when a hook was consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JudgeOutcome {
    /// The hook was not used; the result equals the call without a hook.
    Skipped {
        /// The log code.
        reason: String,
    },
    /// The hook answered and its answer was applied.
    Applied {
        /// How many questions were put to it.
        checked: usize,
        /// How many of them changed the result.
        changed: usize,
    },
}

fn skipped(reason: impl Into<String>) -> JudgeOutcome {
    JudgeOutcome::Skipped {
        reason: reason.into(),
    }
}

// ---------------------------------------------------------------- facts

/// One accepted fact put to a [`FactJudge`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FactClaim {
    /// The chunk it came from.
    pub chunk: usize,
    /// The frontmatter key.
    pub key: String,
    /// The predicate IRI the key asserts.
    pub predicate: Iri,
    /// The label of the note the fact is about (the subject).
    pub subject: String,
    /// The value exactly as the patch would write it.
    pub written: String,
    /// The note text the engine located as evidence.
    pub quote: String,
}

/// A judge's verdict on one claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The quote entails the fact.
    Supported,
    /// The quote does not entail the fact, with enough confidence to act.
    Unsupported,
    /// No decision (low confidence or an unusable answer). Treated as
    /// "keep", so the result is the plain one.
    Undecided,
}

/// Semantic support check for accepted facts. Implemented by the host.
pub trait FactJudge {
    /// Judge every claim. The answer must have one verdict per claim, in
    /// order; anything else is treated as malformed.
    ///
    /// # Errors
    /// [`Unavailable`] when no usable answer exists.
    fn judge(&self, claims: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable>;
}

/// The deterministic default: never available.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoJudge;

impl FactJudge for NoJudge {
    fn judge(&self, _claims: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable> {
        Err(Unavailable::new("flag-off"))
    }
}

/// The result of [`Ingest::accept_judged`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Judged {
    /// The candidates. With the judge skipped these equal
    /// [`Ingest::accept`]'s.
    pub candidates: Candidates,
    /// Whether and how the judge was used.
    pub outcome: JudgeOutcome,
}

fn same_fact(a: &crate::CandidateAssertion, key: &str, written: &str) -> bool {
    a.key == key && a.written == written
}

// ------------------------------------------------------------ selection

/// A value/quote pair generated by host code (not by a language model).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCandidate {
    /// The chunk it was found in.
    pub chunk: usize,
    /// A key from [`ModelRequest::keys`].
    pub key: String,
    /// A name or literal.
    pub value: String,
    /// A verbatim quote from the chunk.
    pub evidence: String,
}

/// One selection question: several candidates for one key in one chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SelectionQuestion {
    /// Label of the note the facts are about.
    pub subject: String,
    /// The key being filled.
    pub key: String,
    /// The predicate IRI of the key.
    pub predicate: Iri,
    /// The chunk index.
    pub chunk: usize,
    /// The chunk text.
    pub text: String,
    /// The candidates, in the order the host supplied them.
    pub candidates: Vec<HostCandidate>,
}

/// A selector's answer to one question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// Keep only the candidate at this index.
    Pick(usize),
    /// The chunk states none of the candidates; keep none.
    NotStated,
    /// No decision; keep every candidate.
    Undecided,
}

/// Choose among host-generated candidates. Implemented by the host.
pub trait CandidateSelector {
    /// One answer per question, in order.
    ///
    /// # Errors
    /// [`Unavailable`] when no usable answer exists.
    fn select(&self, questions: &[SelectionQuestion]) -> Result<Vec<Selection>, Unavailable>;
}

/// The deterministic default: never available.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSelector;

impl CandidateSelector for NoSelector {
    fn select(&self, _questions: &[SelectionQuestion]) -> Result<Vec<Selection>, Unavailable> {
        Err(Unavailable::new("flag-off"))
    }
}

/// The result of [`Ingest::select_responses`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Selected {
    /// Responses to hand to [`Ingest::accept`], one per chunk that has any
    /// candidate, in chunk order. Quote location, grounding and every other
    /// check happen in `accept`, exactly as for a model's responses.
    pub responses: Vec<ModelResponse>,
    /// Whether and how the selector was used.
    pub outcome: JudgeOutcome,
}

// ----------------------------------------------------------- suggestion

/// A vocabulary term a note mentions, offered to a [`FieldSuggester`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SuggestionCandidate {
    /// The term's IRI.
    pub iri: Iri,
    /// Its label.
    pub label: String,
    /// Its kind.
    pub kind: Kind,
}

/// What a [`FieldSuggester`] is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SuggestionQuestion {
    /// Label of the note.
    pub subject: String,
    /// The start of the note body (at most 2,000 characters).
    pub excerpt: String,
    /// The name-valued keys that can be filled on this note.
    pub keys: Vec<KeySpec>,
    /// Vocabulary terms whose names occur in the note.
    pub candidates: Vec<SuggestionCandidate>,
}

/// One raw suggestion from a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSuggestion {
    /// A key from [`SuggestionQuestion::keys`].
    pub key: String,
    /// A candidate's IRI.
    pub target: Iri,
    /// The host judged the choice ambiguous.
    pub ambiguous: bool,
}

/// Suggest explicit frontmatter fields. Implemented by the host.
pub trait FieldSuggester {
    /// Suggest fields; an empty list means "not stated".
    ///
    /// # Errors
    /// [`Unavailable`] when no usable answer exists.
    fn suggest(&self, question: &SuggestionQuestion) -> Result<Vec<RawSuggestion>, Unavailable>;
}

/// The deterministic default: never available.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSuggester;

impl FieldSuggester for NoSuggester {
    fn suggest(&self, _q: &SuggestionQuestion) -> Result<Vec<RawSuggestion>, Unavailable> {
        Err(Unavailable::new("flag-off"))
    }
}

/// A validated suggestion: a proposal for a reviewed edit, not a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldSuggestion {
    /// The frontmatter key.
    pub key: String,
    /// The key's predicate.
    pub predicate: Iri,
    /// The existing entity it points at.
    pub target: Iri,
    /// That entity's label.
    pub label: String,
    /// The value as it would be written (`[[Note]]`).
    pub written: String,
}

/// The result of [`Ingest::suggest_fields`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Suggestions {
    /// Valid proposals.
    pub proposals: Vec<FieldSuggestion>,
    /// Valid proposals the host flagged ambiguous; show, never apply.
    pub ambiguous: Vec<FieldSuggestion>,
    /// Whether and how the suggester was used. When skipped both lists are
    /// empty, which is what the engine returned before this hook existed.
    pub outcome: JudgeOutcome,
}

const EXCERPT_CHARS: usize = 2000;
const MAX_CANDIDATES: usize = 25;

/// Lower-cased words of `text` with punctuation read as a space, so a name
/// at the end of a sentence still matches.
fn words(text: &str) -> String {
    normalize_text(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn bare_target(link: &str) -> String {
    let t = link.trim();
    let t = t.strip_prefix("[[").unwrap_or(t);
    let t = t.strip_suffix("]]").unwrap_or(t);
    t.split('|').next().unwrap_or(t).trim().to_owned()
}

impl Ingest<'_> {
    /// [`Ingest::accept`], then ask `judge` whether each accepted fact's
    /// quote supports it, and withhold those it confidently says do not.
    ///
    /// The plain result is returned unchanged when the judge is
    /// unavailable, answers with the wrong number of verdicts, supports or
    /// is undecided about everything, or when dropping the rejected
    /// proposals would let the re-run write a fact the plain run did not
    /// (for example a competing value becoming the only one left).
    ///
    /// # Errors
    /// The same errors as [`Ingest::accept`]; the judge adds none.
    pub fn accept_judged(
        &self,
        doc: &Document,
        request: &ModelRequest,
        responses: &[ModelResponse],
        judge: &dyn FactJudge,
    ) -> Result<Judged, IngestError> {
        let base = self.accept(doc, request, responses)?;
        let plain = |base: Candidates, why: &str| Judged {
            candidates: base,
            outcome: skipped(why),
        };
        if base.accepted.is_empty() {
            return Ok(plain(base, "nothing-to-judge"));
        }

        // Map each proposal to the fact it yields on its own; only proposals
        // that the plain run accepted are ever put to the judge.
        struct Probe {
            response: usize,
            assertion: usize,
            key: String,
            written: String,
        }
        let mut probes: Vec<Probe> = Vec::new();
        for (ri, r) in responses.iter().enumerate() {
            for (ai, a) in r.assertions.iter().enumerate() {
                let single = ModelResponse {
                    chunk: r.chunk,
                    assertions: vec![a.clone()],
                };
                let Ok(one) = self.accept(doc, request, std::slice::from_ref(&single)) else {
                    continue;
                };
                let Some(fact) = one.accepted.first() else {
                    continue;
                };
                if base
                    .accepted
                    .iter()
                    .any(|b| same_fact(b, &fact.key, &fact.written))
                {
                    probes.push(Probe {
                        response: ri,
                        assertion: ai,
                        key: fact.key.clone(),
                        written: fact.written.clone(),
                    });
                }
            }
        }

        // One claim per distinct fact.
        let mut claims: Vec<FactClaim> = Vec::new();
        let mut facts: Vec<(String, String)> = Vec::new();
        for fact in &base.accepted {
            let Some(ev) = fact.evidence.first() else {
                continue;
            };
            facts.push((fact.key.clone(), fact.written.clone()));
            claims.push(FactClaim {
                chunk: ev.chunk,
                key: fact.key.clone(),
                predicate: fact.predicate.clone(),
                subject: request.note.label.clone(),
                written: fact.written.clone(),
                quote: ev.quote.clone(),
            });
        }
        if claims.is_empty() {
            return Ok(plain(base, "nothing-to-judge"));
        }
        let verdicts = match judge.judge(&claims) {
            Ok(v) if v.len() == claims.len() => v,
            Ok(_) => return Ok(plain(base, "malformed")),
            Err(u) => return Ok(plain(base, &u.reason)),
        };
        let rejected: Vec<&(String, String)> = facts
            .iter()
            .zip(&verdicts)
            .filter(|(_, v)| **v == Verdict::Unsupported)
            .map(|(f, _)| f)
            .collect();
        if rejected.is_empty() {
            return Ok(Judged {
                candidates: base,
                outcome: JudgeOutcome::Applied {
                    checked: claims.len(),
                    changed: 0,
                },
            });
        }

        let dropped: Vec<&Probe> = probes
            .iter()
            .filter(|p| rejected.iter().any(|(k, w)| *k == p.key && *w == p.written))
            .collect();
        let filtered: Vec<ModelResponse> = responses
            .iter()
            .enumerate()
            .map(|(ri, r)| ModelResponse {
                chunk: r.chunk,
                assertions: r
                    .assertions
                    .iter()
                    .enumerate()
                    .filter(|(ai, _)| {
                        !dropped
                            .iter()
                            .any(|p| p.response == ri && p.assertion == *ai)
                    })
                    .map(|(_, a)| a.clone())
                    .collect(),
            })
            .collect();
        let mut after = self.accept(doc, request, &filtered)?;
        let monotone = after.accepted.iter().all(|a| {
            base.accepted
                .iter()
                .any(|b| same_fact(b, &a.key, &a.written))
        });
        if !monotone {
            return Ok(plain(base, "not-monotone"));
        }
        for p in &dropped {
            let a = &responses[p.response].assertions[p.assertion];
            let evidence = base
                .accepted
                .iter()
                .find(|b| same_fact(b, &p.key, &p.written))
                .and_then(|b| b.evidence.first().cloned());
            after.withheld.push(Withheld {
                chunk: responses[p.response].chunk,
                key: a.key.clone(),
                value: a.value.clone(),
                reason: WithholdReason::JudgedUnsupported,
                evidence,
            });
        }
        Ok(Judged {
            candidates: after,
            outcome: JudgeOutcome::Applied {
                checked: claims.len(),
                changed: rejected.len(),
            },
        })
    }

    /// Turn host-generated candidates into [`ModelResponse`]s, asking
    /// `selector` to narrow each (chunk, key) group.
    ///
    /// The baseline — and the result whenever the selector is unavailable,
    /// malformed or undecided — is every candidate, in the order given,
    /// grouped by chunk. The selector can only keep one candidate of a group
    /// or none of it; it cannot add, rewrite or reorder a value or quote.
    /// Hand the responses to [`Ingest::accept`], which stays the final
    /// validator.
    ///
    /// # Errors
    /// [`IngestError::UnknownChunk`] when a candidate names a chunk the
    /// request does not have.
    pub fn select_responses(
        &self,
        request: &ModelRequest,
        candidates: &[HostCandidate],
        selector: &dyn CandidateSelector,
    ) -> Result<Selected, IngestError> {
        for c in candidates {
            if c.chunk >= request.chunks.len() {
                return Err(IngestError::UnknownChunk {
                    chunk: c.chunk,
                    count: request.chunks.len(),
                });
            }
        }
        let assemble = |keep: &dyn Fn(&HostCandidate) -> bool| -> Vec<ModelResponse> {
            let mut by_chunk: IndexMap<usize, Vec<crate::ProposedAssertion>> = IndexMap::new();
            for c in candidates.iter().filter(|c| keep(c)) {
                by_chunk
                    .entry(c.chunk)
                    .or_default()
                    .push(crate::ProposedAssertion {
                        key: c.key.clone(),
                        value: c.value.clone(),
                        evidence: c.evidence.clone(),
                    });
            }
            by_chunk.sort_keys();
            by_chunk
                .into_iter()
                .map(|(chunk, assertions)| ModelResponse { chunk, assertions })
                .collect()
        };
        let baseline = assemble(&|_| true);
        let done = |outcome| Selected {
            responses: baseline.clone(),
            outcome,
        };
        if candidates.is_empty() {
            return Ok(done(skipped("nothing-to-select")));
        }

        let mut groups: IndexMap<(usize, String), Vec<usize>> = IndexMap::new();
        for (i, c) in candidates.iter().enumerate() {
            groups.entry((c.chunk, c.key.clone())).or_default().push(i);
        }
        let mut questions = Vec::new();
        let mut order = Vec::new();
        for ((chunk, key), idxs) in &groups {
            // A key the request does not grant is left for `accept` to refuse.
            let Some(spec) = request.keys.iter().find(|k| &k.key == key) else {
                continue;
            };
            questions.push(SelectionQuestion {
                subject: request.note.label.clone(),
                key: key.clone(),
                predicate: spec.predicate.clone(),
                chunk: *chunk,
                text: request.chunks[*chunk].text.clone(),
                candidates: idxs.iter().map(|i| candidates[*i].clone()).collect(),
            });
            order.push(idxs.clone());
        }
        if questions.is_empty() {
            return Ok(done(skipped("nothing-to-select")));
        }
        let answers = match selector.select(&questions) {
            Ok(a) if a.len() == questions.len() => a,
            Ok(_) => return Ok(done(skipped("malformed"))),
            Err(u) => return Ok(done(skipped(&u.reason))),
        };
        let mut removed: Vec<usize> = Vec::new();
        let mut changed = 0;
        for (idxs, answer) in order.iter().zip(&answers) {
            match answer {
                Selection::Undecided => {}
                Selection::NotStated => {
                    removed.extend(idxs.iter().copied());
                    changed += 1;
                }
                Selection::Pick(p) if *p < idxs.len() => {
                    removed.extend(idxs.iter().copied().filter(|i| *i != idxs[*p]));
                    if idxs.len() > 1 {
                        changed += 1;
                    }
                }
                // An index outside the offered set is malformed: keep all.
                Selection::Pick(_) => {}
            }
        }
        let kept = |c: &HostCandidate| {
            let i = candidates
                .iter()
                .position(|x| std::ptr::eq(x, c))
                .unwrap_or(usize::MAX);
            !removed.contains(&i)
        };
        Ok(Selected {
            responses: assemble(&kept),
            outcome: JudgeOutcome::Applied {
                checked: questions.len(),
                changed,
            },
        })
    }

    /// Vocabulary terms whose label, alias or note name occurs in the body of
    /// `doc`, excluding the note's own entity and conflicted terms, sorted by
    /// IRI and capped at 25.
    #[must_use]
    pub fn mentioned_terms(&self, id: &DocId, doc: &Document) -> Vec<&Term> {
        let own = self.model.entity_of(id).map(|e| e.iri.clone());
        let body = format!(" {} ", words(doc.body()));
        self.vocabulary
            .terms()
            .iter()
            .filter(|t| Some(&t.iri) != own.as_ref() && !t.conflicted)
            .filter(|t| {
                std::iter::once(t.label.as_str())
                    .chain(t.aliases.iter().map(String::as_str))
                    .chain(std::iter::once(t.note_name()))
                    .map(|n| words(&normalize_label(n)))
                    .any(|n| !n.is_empty() && body.contains(&format!(" {n} ")))
            })
            .take(MAX_CANDIDATES)
            .collect()
    }

    /// Ask `suggester` which existing vocabulary terms fill which
    /// name-valued keys of a note.
    ///
    /// Every answer is validated here: the key must be one the request
    /// grants (reserved keys such as `public` never are), the target must be
    /// one of the mentioned terms offered, its kind must fit the predicate,
    /// a link must resolve back to it alone, and values the note already
    /// holds are dropped. The result is a proposal for a reviewed edit; no
    /// patch is built and no entity is minted. When the suggester is
    /// unavailable both lists are empty.
    ///
    /// # Errors
    /// [`IngestError::NotAnEntity`] / [`IngestError::NoAssertionKeys`] as
    /// for [`Ingest::plan`].
    pub fn suggest_fields(
        &self,
        id: &DocId,
        doc: &Document,
        suggester: &dyn FieldSuggester,
    ) -> Result<Suggestions, IngestError> {
        let request = self.plan(id, doc)?;
        let none = |why: &str| Suggestions {
            proposals: Vec::new(),
            ambiguous: Vec::new(),
            outcome: skipped(why),
        };
        let terms = self.mentioned_terms(id, doc);
        let keys: Vec<KeySpec> = request
            .keys
            .iter()
            .filter(|k| matches!(k.value, ValueKind::Name))
            .cloned()
            .collect();
        if terms.is_empty() || keys.is_empty() {
            return Ok(none("nothing-to-suggest"));
        }
        let question = SuggestionQuestion {
            subject: request.note.label.clone(),
            excerpt: doc.body().chars().take(EXCERPT_CHARS).collect(),
            keys,
            candidates: terms
                .iter()
                .map(|t| SuggestionCandidate {
                    iri: t.iri.clone(),
                    label: t.label.clone(),
                    kind: t.kind,
                })
                .collect(),
        };
        let raw = match suggester.suggest(&question) {
            Ok(r) => r,
            Err(u) => return Ok(none(&u.reason)),
        };
        let checked = raw.len();
        let mut proposals: Vec<FieldSuggestion> = Vec::new();
        let mut ambiguous: Vec<FieldSuggestion> = Vec::new();
        for r in raw {
            let Some(spec) = question.keys.iter().find(|k| k.key == r.key) else {
                continue;
            };
            let Some(term) = terms.iter().find(|t| t.iri == r.target) else {
                continue;
            };
            if let Some(expected) = expected_kind(spec.form, request.note.kind) {
                if !kind_fits(expected, term.kind) {
                    continue;
                }
            }
            let Ok(target) = link_target(self.profile, self.model, term, &term.iri) else {
                continue;
            };
            let written = format!("[[{target}]]");
            if spec
                .present
                .iter()
                .any(|p| bare_target(p) == target || p.trim() == written)
            {
                continue;
            }
            let list = if r.ambiguous {
                &mut ambiguous
            } else {
                &mut proposals
            };
            if list
                .iter()
                .any(|s| s.key == spec.key && s.target == term.iri)
            {
                continue;
            }
            list.push(FieldSuggestion {
                key: spec.key.clone(),
                predicate: spec.predicate.clone(),
                target: term.iri.clone(),
                label: term.label.clone(),
                written,
            });
        }
        let changed = proposals.len() + ambiguous.len();
        Ok(Suggestions {
            proposals,
            ambiguous,
            outcome: JudgeOutcome::Applied { checked, changed },
        })
    }
}
