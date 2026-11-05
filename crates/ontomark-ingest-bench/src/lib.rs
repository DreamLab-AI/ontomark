//! A labelled extraction benchmark for `ontomark-ingest`.
//!
//! Every case is one note with one statement and one candidate fact. The
//! quote occurs verbatim in the note and names an existing vocabulary term,
//! so the fact is *syntactically groundable*; the label says whether the
//! quote actually *states* it. Adversarial categories (negated, hypothetical,
//! historical, attributed, uncertain, contradicted) keep the words and drop
//! the assertion.
//!
//! Three arms are scored against the same labels:
//!
//! | Arm | Proposals | Decision |
//! |---|---|---|
//! | `deterministic` | the case's own fact (an extractor that proposes everything mentioned) | [`Ingest::accept`] |
//! | `model` | a chat model, prompted exactly as the Obsidian plugin prompts it | [`Ingest::accept`] |
//! | `model+jev` | as `model` | plus the Jev judge's support probability, swept over withholding thresholds |
//!
//! A fact counts as predicted *true* when it reaches the accepted set. The
//! deterministic arm therefore measures what grounding alone cannot tell
//! apart, not a flaw: grounding was never meant to read negation.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ontomark_ingest::{
    CandidateAssertion, FactClaim, FactJudge, Ingest, ModelRequest, ModelResponse, Options,
    ProposedAssertion, Unavailable, Verdict,
};
use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
use ontomark_parse::Document;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The IRI namespace of the benchmark vocabulary.
pub const NS: &str = "urn:bench:";

/// The labelled dataset shipped with the crate.
pub const CASES: &str = include_str!("../fixtures/cases.json");

/// The dataset file.
#[derive(Debug, Clone, Deserialize)]
pub struct Dataset {
    /// Format version.
    pub version: u32,
    /// The class names every case's vault holds.
    pub vocabulary: Vec<String>,
    /// The cases.
    pub cases: Vec<Case>,
}

/// One labelled case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    /// Stable id, `<category>-<nn>`.
    pub id: String,
    /// The statement category.
    pub category: String,
    /// The note the fact is about.
    pub subject: String,
    /// The note body.
    pub body: String,
    /// The frontmatter key.
    pub key: String,
    /// The value: a vocabulary name, or a literal.
    pub value: String,
    /// The verbatim quote.
    pub quote: String,
    /// Does the quote state the fact?
    pub supported: bool,
}

/// Parse [`CASES`].
///
/// # Panics
/// If the shipped fixture is malformed (a build defect, caught by the tests).
#[must_use]
pub fn dataset() -> Dataset {
    serde_json::from_str(CASES).expect("fixtures/cases.json parses")
}

/// The profile every case uses: classes, a subclass key, a part-of
/// existential and a founding date.
///
/// # Panics
/// If the built-in spec is invalid (a build defect).
#[must_use]
pub fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_value(json!({
        "id": "ingest-bench/1",
        "kinds": { "Class": "class" },
        "predicates": {
            "is-a": { "iri": "http://www.w3.org/2000/01/rdf-schema#subClassOf", "form": "sub_class_of" },
            "part-of": { "iri": format!("{NS}partOf"), "form": "existential" },
            "founded": {
                "iri": format!("{NS}founded"),
                "form": "data_assertion",
                "datatype": "http://www.w3.org/2001/XMLSchema#date"
            }
        },
        "reserved": ["tags", "public"]
    }))
    .expect("bench profile spec");
    DeclarativeProfile::new(spec).expect("bench profile")
}

fn iri_of(name: &str) -> String {
    format!("{NS}{}", name.replace(' ', "_"))
}

/// One case's vault: the vocabulary pages plus the subject note.
#[must_use]
pub fn vault(vocabulary: &[String], case: &Case) -> (Model, DocId, Document) {
    let page = |name: &str, body: &str| {
        (
            DocId::new(format!("{name}.md")),
            Document::parse(format!(
                "---\ntype: Class\niri: {}\n---\n{body}",
                iri_of(name)
            )),
        )
    };
    let mut docs: Vec<(DocId, Document)> = vocabulary.iter().map(|v| page(v, "")).collect();
    let subject = page(&case.subject, &format!("{}\n", case.body));
    docs.push(subject.clone());
    let model = Model::build(&profile(), docs.iter().map(|(i, d)| (i.clone(), d)));
    (model, subject.0, subject.1)
}

/// Produces model responses for a planned request.
pub trait Extractor {
    /// The arm's name in reports.
    fn name(&self) -> &str;
    /// Responses for `request`, which was planned for `case`.
    ///
    /// # Errors
    /// A message when the extractor could not answer.
    fn extract(&self, request: &ModelRequest, case: &Case) -> Result<Vec<ModelResponse>, String>;
}

/// Proposes exactly the case's fact: the upper bound of what grounding
/// alone lets through.
#[derive(Debug, Clone, Copy, Default)]
pub struct Oracle;

impl Extractor for Oracle {
    fn name(&self) -> &str {
        "deterministic"
    }

    fn extract(&self, request: &ModelRequest, case: &Case) -> Result<Vec<ModelResponse>, String> {
        let chunk = request
            .chunks
            .iter()
            .find(|c| c.text.contains(&case.quote))
            .ok_or_else(|| format!("{}: quote not in any chunk", case.id))?;
        Ok(vec![ModelResponse::new(
            chunk.index,
            vec![ProposedAssertion::new(&case.key, &case.value, &case.quote)],
        )])
    }
}

/// An OpenAI-compatible chat endpoint, prompted as the Obsidian plugin
/// prompts it (`src/ingest-transport.ts`: system = instructions, user = keys,
/// schema and chunk, `json_object`, temperature 0).
#[derive(Debug, Clone)]
pub struct ChatModel {
    /// The `/chat/completions` URL.
    pub url: String,
    /// The model name.
    pub model: String,
    /// A bearer key, if the endpoint needs one.
    pub bearer: Option<String>,
}

fn describe_key(k: &ontomark_ingest::KeySpec) -> String {
    let kind = match &k.value {
        ontomark_ingest::ValueKind::Literal { datatype } => format!("literal ({datatype})"),
        _ => "name".to_owned(),
    };
    let present = if k.present.is_empty() {
        String::new()
    } else {
        format!("; already present: {}", k.present.join(", "))
    };
    format!("- {}: {kind}{present}", k.key)
}

fn user_message(request: &ModelRequest, chunk: &ontomark_ingest::Chunk) -> String {
    let under = if chunk.heading_path.is_empty() {
        String::new()
    } else {
        format!("\nUnder: {}", chunk.heading_path.join(" > "))
    };
    let mut lines = vec![
        format!("Note: {}", request.note.label),
        "Keys you may fill:".to_owned(),
    ];
    lines.extend(request.keys.iter().map(describe_key));
    lines.push(String::new());
    lines.push(format!(
        "Response schema (answer with one JSON object, chunk = {}):",
        chunk.index
    ));
    lines.push(request.response_schema.to_string());
    lines.push(String::new());
    lines.push(format!("Chunk {}:{under}", chunk.index));
    lines.push(chunk.text.clone());
    lines.join("\n")
}

fn parse_answer(content: &str, chunk: usize) -> Option<ModelResponse> {
    let t = content.trim();
    let t = t
        .strip_prefix("```json")
        .or_else(|| t.strip_prefix("```"))
        .map_or(t, |r| r.trim_end().trim_end_matches("```").trim());
    let v: Value = serde_json::from_str(t).ok()?;
    let mut out = Vec::new();
    for a in v.get("assertions")?.as_array()? {
        let s = |k: &str| a.get(k).and_then(Value::as_str).map(str::to_owned);
        out.push(ProposedAssertion::new(
            s("key")?,
            s("value")?,
            s("evidence")?,
        ));
    }
    Some(ModelResponse::new(chunk, out))
}

impl Extractor for ChatModel {
    fn name(&self) -> &str {
        "model"
    }

    fn extract(&self, request: &ModelRequest, _case: &Case) -> Result<Vec<ModelResponse>, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(600))
            .redirects(0)
            .build();
        let mut out = Vec::new();
        for chunk in &request.chunks {
            let body = json!({
                "model": self.model,
                "messages": [
                    { "role": "system", "content": request.instructions },
                    { "role": "user", "content": user_message(request, chunk) },
                ],
                "response_format": { "type": "json_object" },
                "temperature": 0,
                "max_tokens": 1536,
            });
            let mut req = agent.post(&self.url);
            if let Some(b) = &self.bearer {
                req = req.set("Authorization", &format!("Bearer {b}"));
            }
            let reply: Value = req
                .send_json(body)
                .map_err(|e| e.to_string())?
                .into_json()
                .map_err(|e| e.to_string())?;
            let content = reply
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            // A malformed answer drops the chunk, as the plugin does.
            if let Some(r) = parse_answer(content, chunk.index) {
                out.push(r);
            }
        }
        Ok(out)
    }
}

/// Support probabilities for accepted facts (the Jev judge, or a stand-in).
pub type SupportFn<'a> = dyn Fn(&[FactClaim]) -> Result<Vec<f64>, String> + 'a;

/// What one arm did with one case.
#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    /// Case id.
    pub id: String,
    /// Case category.
    pub category: String,
    /// The label.
    pub supported: bool,
    /// Did the labelled fact reach the accepted set?
    pub accepted: bool,
    /// Accepted facts other than the labelled one (unlabelled extras).
    pub extra_accepted: usize,
    /// Jev's support probability for the labelled fact, when it was accepted
    /// and a judge ran.
    pub support: Option<f64>,
    /// Why the case could not be scored, if it could not.
    pub error: Option<String>,
}

fn is_labelled(a: &CandidateAssertion, case: &Case) -> bool {
    if a.key != case.key {
        return false;
    }
    match &a.grounded {
        Some(iri) => iri.as_str() == iri_of(&case.value),
        None => a.written.trim_matches('"') == case.value,
    }
}

/// A judge that records each claim's support and decides nothing, so
/// [`Ingest::accept_judged`] returns exactly the plain candidates while the
/// claims are the ones production builds.
struct Recorder<'a> {
    support: &'a SupportFn<'a>,
    seen: RefCell<Vec<(String, String, f64)>>,
    error: RefCell<Option<String>>,
}

impl FactJudge for Recorder<'_> {
    fn judge(&self, claims: &[FactClaim]) -> Result<Vec<Verdict>, Unavailable> {
        match (self.support)(claims) {
            Ok(ps) if ps.len() == claims.len() => {
                let mut seen = self.seen.borrow_mut();
                for (c, p) in claims.iter().zip(ps) {
                    seen.push((c.key.clone(), c.written.clone(), p));
                }
                Ok(vec![Verdict::Undecided; claims.len()])
            }
            Ok(_) => {
                *self.error.borrow_mut() = Some("judge: wrong number of answers".into());
                Err(Unavailable::new("malformed"))
            }
            Err(e) => {
                *self.error.borrow_mut() = Some(format!("judge: {e}"));
                Err(Unavailable::new("unavailable"))
            }
        }
    }
}

/// Run one case through `extractor`, then `support` if given.
pub fn run_case(
    vocabulary: &[String],
    case: &Case,
    extractor: &dyn Extractor,
    support: Option<&SupportFn<'_>>,
) -> CaseResult {
    let mut result = CaseResult {
        id: case.id.clone(),
        category: case.category.clone(),
        supported: case.supported,
        accepted: false,
        extra_accepted: 0,
        support: None,
        error: None,
    };
    let profile = profile();
    let (model, id, doc) = vault(vocabulary, case);
    let ingest = Ingest::new(&profile, &model, Options::default());
    let recorder = support.map(|support| Recorder {
        support,
        seen: RefCell::new(Vec::new()),
        error: RefCell::new(None),
    });
    let outcome = ingest
        .plan(&id, &doc)
        .map_err(|e| e.to_string())
        .and_then(|request| {
            let responses = extractor.extract(&request, case)?;
            match &recorder {
                Some(r) => ingest
                    .accept_judged(&doc, &request, &responses, r)
                    .map(|j| j.candidates),
                None => ingest.accept(&doc, &request, &responses),
            }
            .map_err(|e| e.to_string())
        });
    let candidates = match outcome {
        Ok(c) => c,
        Err(e) => {
            result.error = Some(e);
            return result;
        }
    };
    let labelled = candidates.accepted.iter().find(|a| is_labelled(a, case));
    result.accepted = labelled.is_some();
    result.extra_accepted = candidates.accepted.len() - usize::from(labelled.is_some());
    if let (Some(a), Some(r)) = (labelled, &recorder) {
        if let Some(e) = r.error.borrow_mut().take() {
            result.error = Some(e);
        }
        result.support = r
            .seen
            .borrow()
            .iter()
            .find(|(k, w, _)| *k == a.key && *w == a.written)
            .map(|(_, _, p)| *p);
    }
    result
}

/// A confusion matrix and its rates.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct Score {
    /// Supported and accepted.
    pub tp: usize,
    /// Unsupported but accepted.
    pub fp: usize,
    /// Supported but not accepted.
    pub r#fn: usize,
    /// Unsupported and not accepted.
    pub tn: usize,
    /// Cases that could not be scored (excluded from the counts above).
    pub errors: usize,
}

impl Score {
    /// Score `results`, treating a fact as kept when `kept` says so.
    pub fn of<'a>(
        results: impl IntoIterator<Item = &'a CaseResult>,
        kept: impl Fn(&CaseResult) -> bool,
    ) -> Self {
        let mut s = Self::default();
        for r in results {
            if r.error.is_some() {
                s.errors += 1;
                continue;
            }
            match (r.supported, kept(r)) {
                (true, true) => s.tp += 1,
                (false, true) => s.fp += 1,
                (true, false) => s.r#fn += 1,
                (false, false) => s.tn += 1,
            }
        }
        s
    }

    /// TP / (TP + FP); `None` when nothing was kept.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        ratio(self.tp, self.tp + self.fp)
    }

    /// TP / (TP + FN); `None` when there are no positives.
    #[must_use]
    pub fn recall(&self) -> Option<f64> {
        ratio(self.tp, self.tp + self.r#fn)
    }

    /// Share of unsupported facts that were kept: the "risk" a reviewer
    /// inherits.
    #[must_use]
    pub fn false_acceptance(&self) -> Option<f64> {
        ratio(self.fp, self.fp + self.tn)
    }

    /// The rates as JSON (`null` where undefined).
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "tp": self.tp, "fp": self.fp, "fn": self.r#fn, "tn": self.tn, "errors": self.errors,
            "precision": self.precision(), "recall": self.recall(),
            "false_acceptance": self.false_acceptance(),
        })
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(a: usize, b: usize) -> Option<f64> {
    (b > 0).then(|| a as f64 / b as f64)
}

/// Overall and per-category scores of `results` under `kept`.
pub fn breakdown(results: &[CaseResult], kept: impl Fn(&CaseResult) -> bool + Copy) -> Value {
    let mut by: BTreeMap<&str, Vec<&CaseResult>> = BTreeMap::new();
    for r in results {
        by.entry(r.category.as_str()).or_default().push(r);
    }
    json!({
        "overall": Score::of(results, kept).to_json(),
        "by_category": by.into_iter().map(|(k, v)| (k.to_owned(), Score::of(v, kept).to_json())).collect::<serde_json::Map<_, _>>(),
    })
}

/// Withholding thresholds swept for the Jev arm (a fact is withheld when its
/// support is at or below the threshold; 0 withholds nothing, as in the
/// plugin and CLI).
pub const THRESHOLDS: [f64; 10] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];

/// The selective-risk curve: for each threshold, the score when facts with
/// support at or below it are withheld. A fact the judge did not score is
/// kept (the judge fails open).
#[must_use]
pub fn risk_curve(results: &[CaseResult]) -> Vec<Value> {
    THRESHOLDS
        .iter()
        .map(|&t| {
            let kept =
                |r: &CaseResult| r.accepted && !(t > 0.0 && r.support.is_some_and(|p| p <= t));
            let s = Score::of(results, kept);
            let kept_n = s.tp + s.fp;
            json!({ "withhold_at_or_below": t, "kept": kept_n, "score": s.to_json() })
        })
        .collect()
}
