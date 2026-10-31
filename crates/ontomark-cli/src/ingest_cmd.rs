//! `ontomark ingest`: external-source evidence becomes a proposal bundle
//! (ADR-OM-010 decisions 3 and 4).
//!
//! The command reads sources through `ontomark-source`, picks target notes,
//! asks a model for names and quotes through `ontomark-ingest`, and writes a
//! bundle: `manifest/<id>.md` (each target page in full with the accepted
//! patch applied, the shape `vault propose --diff <DIR>` accepts) and
//! `evidence.json` (quotes, spans, source versions, withheld values and the
//! conflicts a reviewer must decide). It never writes to the vault and never
//! runs `vault propose`; it prints the command.
//!
//! * **Endpoint.** Explicit, OpenAI-compatible, and local-only by default:
//!   the policy is `ontomark-jev`'s (`ONTOMARK_JEV_ALLOW_CLOUD=1` opts in).
//! * **Budget.** `--budget-calls` and `--budget-chars` are checked before each
//!   model call; a cached answer costs nothing.
//! * **Cache.** Every answer is stored under the ledger's directory, keyed by
//!   the digest of the request body, the model and the endpoint, so a re-run
//!   with nothing changed makes no model call.
//! * **Request id.** A digest of the sorted source versions, target digests,
//!   model and options; the same inputs reuse the same bundle directory.
//! * **Exit codes.** `0` done (also when the budget stopped the run, which
//!   `budget_exhausted` reports), `1` the model endpoint failed, `2` refused
//!   (cloud endpoint without the opt-in, unknown or ambiguous target, no
//!   usable source, a layout without the requested vault).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context as _;
use clap::Args;
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary as VaultVocabulary;
use ontomark_compile::profile::Profile as VaultProfile;
use ontomark_ingest::normalize::normalize_label;
use ontomark_ingest::{
    Candidates, Chunk, Ingest, KeySpec, ModelRequest, ModelResponse, Options, ProposedAssertion,
    SourceText, ValueKind, WithholdReason, CONTRACT_SOURCE, INSTRUCTIONS_SOURCE,
};
use ontomark_jev::JevConfig;
use ontomark_model::{DocId, Model};
use ontomark_parse::Document;
use ontomark_source::{
    classify, version_of, Change, Fetched, Ledger, LocalFiles, Source, SourceItem,
};
use serde_json::{json, Value};

use crate::ingest_profile;
use crate::migrate_cmd::{vault_kinds, VAULT_VALUES};

/// Most chunks one (source, target) pair may send; a larger source is
/// refused for that pair rather than becoming a runaway bill.
const MAX_CHUNKS: usize = 64;
/// Reasoning models need room to think before they answer.
const MAX_TOKENS: u32 = 1536;
/// Wall-clock bound for one model call.
const CALL_TIMEOUT: Duration = Duration::from_secs(600);

/// `ontomark ingest` arguments.
#[derive(Debug, Args)]
pub struct IngestArgs {
    /// A source: a file, a directory of Markdown/text/HTML, or an `http(s)` URL.
    /// Repeatable.
    #[arg(long = "source", required = true, value_name = "PATH|URL")]
    sources: Vec<String>,
    /// A target note to fill, by page id or exact title. Repeatable.
    #[arg(long = "target", value_name = "NOTE")]
    targets: Vec<String>,
    /// Also target the notes whose label or alias appears verbatim in a
    /// source (exact match on normalised words, never fuzzy).
    #[arg(long)]
    discover: bool,
    /// The most notes `--discover` may add per source.
    #[arg(long, default_value_t = 10, value_name = "N")]
    max_targets: usize,
    /// The OpenAI-compatible chat endpoint (a `/v1` base or the full
    /// `/chat/completions` URL). Local-only unless `ONTOMARK_JEV_ALLOW_CLOUD=1`.
    #[arg(long, required_unless_present = "plan_only")]
    endpoint: Option<String>,
    /// The model name sent in the request.
    #[arg(long, required_unless_present = "plan_only")]
    model: Option<String>,
    /// The environment variable holding a bearer token for the endpoint.
    #[arg(long, value_name = "VAR")]
    key_env: Option<String>,
    /// Stop before a model call that would exceed this many calls.
    #[arg(long, value_name = "N")]
    budget_calls: Option<usize>,
    /// Stop before a model call that would take the characters sent past this.
    #[arg(long, value_name = "N")]
    budget_chars: Option<usize>,
    /// The bundle directory. Default `.ontomark/ingest/<request-id>/`.
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// The source ledger; the response cache sits beside it. Default
    /// `.ontomark/ingest/ledger.json`.
    #[arg(long, value_name = "FILE")]
    ledger: Option<PathBuf>,
    /// Which vault to read targets from. Only `knowledge` is accepted: `working`
    /// and `all` are refused (exit 2) because working notes are private.
    #[arg(long, default_value = "knowledge", value_parser = VAULT_VALUES)]
    vault: String,
    /// Read sources, find targets and plan requests; call no model and write nothing.
    #[arg(long)]
    plan_only: bool,
}

/// A refused request: exit 2.
#[derive(Debug)]
struct Refused(String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn refuse<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(Refused(message.into()).into())
}

/// A source fetched and ready: its qualified identity and rendered text.
struct Fetch {
    item: SourceItem,
    fetched: Fetched,
    text: SourceText,
    change: &'static str,
}

/// One note of the selected vaults.
struct Note {
    id: DocId,
    doc: Document,
    vault: VaultKind,
}

/// Why a run stopped before every pair was done.
enum Stop {
    Budget(&'static str),
    Unavailable(String),
}

/// The model endpoint, budget and counters of one run.
struct Caller {
    url: String,
    model: String,
    bearer: Option<String>,
    cache: PathBuf,
    max_calls: Option<usize>,
    max_chars: Option<usize>,
    calls: usize,
    chars: usize,
    hits: usize,
}

/// Run `ontomark ingest`.
///
/// # Errors
/// Anything that is not a refusal: unreadable vault, bundle not writable.
pub fn run(
    args: &IngestArgs,
    root: &Path,
    vocab: &VaultVocabulary,
    profile: &VaultProfile,
    json: bool,
) -> anyhow::Result<ExitCode> {
    match execute(args, root, vocab, profile, json) {
        Err(e) if e.downcast_ref::<Refused>().is_some() => {
            if json {
                println!("{}", json!({ "refused": e.to_string() }));
            } else {
                eprintln!("ontomark: refused: {e}");
            }
            Ok(ExitCode::from(2))
        }
        other => other,
    }
}

/// The URL to POST to: the base with `/chat/completions` unless it already has it.
fn chat_url(endpoint: &str) -> String {
    let e = endpoint.trim_end_matches('/');
    if e.ends_with("/chat/completions") {
        e.to_owned()
    } else {
        format!("{e}/chat/completions")
    }
}

fn fetch_sources(args: &IngestArgs, ledger: &Ledger, parser: &str) -> anyhow::Result<Vec<Fetch>> {
    let mut out: Vec<Fetch> = Vec::new();
    let mut observed: Vec<(SourceItem, Option<Fetched>)> = Vec::new();
    for spec in &args.sources {
        let (source, qualify): (Box<dyn Source>, bool) =
            if spec.starts_with("http://") || spec.starts_with("https://") {
                (Box::new(ontomark_source::Url::new(spec.clone())), false)
            } else {
                (Box::new(LocalFiles::new(spec)), true)
            };
        let items = source
            .list()
            .map_err(|e| Refused(format!("source {spec}: {e}")))?;
        for mut item in items {
            let fetched = source
                .fetch(&item)
                .map_err(|e| Refused(format!("source {}: {e}", item.locator)))?;
            if qualify {
                let canon = std::fs::canonicalize(&item.locator)
                    .with_context(|| format!("resolving {}", item.locator))?;
                item.id = format!("file:{}", canon.display());
            }
            let mut fetched = fetched;
            fetched.item_id.clone_from(&item.id);
            observed.push((item, Some(fetched)));
        }
    }
    observed.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    observed.dedup_by(|a, b| a.0.id == b.0.id);
    let changes = classify(ledger, &observed, parser);
    for ((item, fetched), change) in observed.into_iter().zip(changes) {
        let fetched = fetched.expect("every observed item was fetched");
        let label = match change {
            Change::New { .. } => "new",
            Change::ContentChanged { .. } => "content_changed",
            Change::ParserChanged { .. } => "parser_changed",
            Change::MarkerOnly { .. } | Change::Unchanged { .. } => "unchanged",
            _ => "unknown",
        };
        let text = SourceText::new(
            ontomark_ingest::SourceRef::new(item.id.clone(), fetched.version.clone()),
            fetched.text.clone(),
        );
        out.push(Fetch {
            item,
            fetched,
            text,
            change: label,
        });
    }
    Ok(out)
}

fn load_notes(
    root: &Path,
    profile: &VaultProfile,
    kinds: &[VaultKind],
) -> anyhow::Result<Vec<Note>> {
    let excluded = profile.layout.excluded_dirs();
    let mut notes: Vec<Note> = Vec::new();
    for kind in kinds {
        let dir = profile.layout.root_of(root, *kind)?;
        let vault = Vault::load_excluding(&dir, *kind, &excluded)
            .with_context(|| format!("loading {}", dir.display()))?;
        for page in vault.pages {
            let text = std::fs::read_to_string(&page.path)
                .with_context(|| format!("reading {}", page.path.display()))?;
            notes.push(Note {
                id: DocId::new(&page.id),
                doc: Document::parse(text),
                vault: *kind,
            });
        }
    }
    notes.sort_by(|a, b| a.id.cmp(&b.id));
    if let Some(w) = notes.windows(2).find(|w| w[0].id == w[1].id) {
        return refuse(format!(
            "page id {} exists in both vaults; ingest one vault at a time",
            w[0].id
        ));
    }
    Ok(notes)
}

/// Notes whose label, alias or note name occurs in `text` as whole normalised
/// words and names exactly one entity. First occurrence first, then id.
fn discover(ingest: &Ingest<'_>, text: &str, max: usize) -> Vec<DocId> {
    let hay = format!(" {} ", normalize_label(text));
    let mut owners: BTreeMap<String, BTreeSet<&DocId>> = BTreeMap::new();
    for term in ingest.vocabulary().terms() {
        let names = std::iter::once(term.label.as_str())
            .chain(term.aliases.iter().map(String::as_str))
            .chain(std::iter::once(term.note_name()));
        for name in names {
            let n = normalize_label(name);
            if n.len() >= 3 {
                owners.entry(n).or_default().insert(&term.document);
            }
        }
    }
    let mut first: BTreeMap<&DocId, usize> = BTreeMap::new();
    for (name, docs) in &owners {
        let [doc] = docs.iter().copied().collect::<Vec<_>>()[..] else {
            continue;
        };
        if let Some(at) = hay.find(&format!(" {name} ")) {
            first
                .entry(doc)
                .and_modify(|p| *p = (*p).min(at))
                .or_insert(at);
        }
    }
    let mut hits: Vec<(usize, &DocId)> = first.into_iter().map(|(d, p)| (p, d)).collect();
    hits.sort();
    hits.into_iter().take(max).map(|(_, d)| d.clone()).collect()
}

/// Resolve `--target` to a note: page id (with or without `.md`), else an
/// exact title or alias that names one entity.
fn resolve_target(ingest: &Ingest<'_>, notes: &[Note], wanted: &str) -> anyhow::Result<DocId> {
    let id = DocId::new(wanted.trim());
    if notes.iter().any(|n| n.id == id) {
        return Ok(id);
    }
    let w = normalize_label(wanted);
    let docs: BTreeSet<&DocId> = ingest
        .vocabulary()
        .terms()
        .iter()
        .filter(|t| {
            std::iter::once(t.label.as_str())
                .chain(t.aliases.iter().map(String::as_str))
                .any(|n| normalize_label(n) == w)
        })
        .map(|t| &t.document)
        .collect();
    match docs.len() {
        1 => Ok(docs.into_iter().next().expect("one").clone()),
        0 => refuse(format!(
            "target {wanted:?} names no page of the selected vault"
        )),
        _ => refuse(format!(
            "target {wanted:?} names several pages; use the page id"
        )),
    }
}

fn user_message(request: &ModelRequest, chunk: &Chunk) -> String {
    let describe = |k: &KeySpec| {
        let kind = match &k.value {
            ValueKind::Name => "name".to_owned(),
            ValueKind::Literal { datatype } => format!("literal ({datatype})"),
            _ => "value".to_owned(),
        };
        let present = if k.present.is_empty() {
            String::new()
        } else {
            format!("; already present: {}", k.present.join(", "))
        };
        format!("- {}: {kind}{present}", k.key)
    };
    let under = if chunk.heading_path.is_empty() {
        String::new()
    } else {
        format!("\nUnder: {}", chunk.heading_path.join(" > "))
    };
    let mut lines = vec![
        format!("Note: {}", request.note.label),
        "Keys you may fill:".to_owned(),
    ];
    lines.extend(request.keys.iter().map(describe));
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

/// The chat-completions body for one chunk. The plugin sends the schema in
/// the prompt with `json_object`; here it is also the `response_format`.
fn request_body(model: &str, request: &ModelRequest, chunk: &Chunk) -> Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": request.instructions },
            { "role": "user", "content": user_message(request, chunk) },
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "ontomark_ingest_facts", "schema": request.response_schema },
        },
        "temperature": 0,
        "max_tokens": MAX_TOKENS,
    })
}

/// Parse the model's message into a response for `chunk`; `None` when it is
/// not the contract shape. The index comes from the request, never the model.
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

fn post_chat(url: &str, bearer: Option<&str>, body: &Value) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(CALL_TIMEOUT)
        .redirects(0)
        .build();
    let mut req = agent.post(url);
    if let Some(b) = bearer {
        req = req.set("Authorization", &format!("Bearer {b}"));
    }
    let resp = match req.send_json(body.clone()) {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => return Err(format!("HTTP {code}")),
        Err(ureq::Error::Transport(t)) => return Err(t.to_string()),
    };
    if !(200..300).contains(&resp.status()) {
        return Err(format!(
            "HTTP {}: the endpoint redirected; point --endpoint at the final URL",
            resp.status()
        ));
    }
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    Ok(v.pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned())
}

impl Caller {
    /// The answer for `body`: from the cache, else from the model within budget.
    fn answer(&mut self, body: &Value) -> Result<(String, bool), Stop> {
        let text = body.to_string();
        let key = version_of(&format!("{text}\0{}\0{}", self.model, self.url));
        let path = self.cache.join(format!("{key}.json"));
        if let Some(c) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.get("content").and_then(Value::as_str).map(str::to_owned))
        {
            self.hits += 1;
            return Ok((c, true));
        }
        let sent: usize = body["messages"].as_array().map_or(0, |m| {
            m.iter()
                .filter_map(|x| x["content"].as_str())
                .map(|c| c.chars().count())
                .sum()
        });
        if self.max_calls.is_some_and(|m| self.calls + 1 > m) {
            return Err(Stop::Budget("budget_calls"));
        }
        if self.max_chars.is_some_and(|m| self.chars + sent > m) {
            return Err(Stop::Budget("budget_chars"));
        }
        self.calls += 1;
        self.chars += sent;
        let content =
            post_chat(&self.url, self.bearer.as_deref(), body).map_err(Stop::Unavailable)?;
        Ok((content, false))
    }

    fn remember(&self, body: &Value, content: &str) {
        let key = version_of(&format!("{body}\0{}\0{}", self.model, self.url));
        let _ = std::fs::create_dir_all(&self.cache);
        let _ = std::fs::write(
            self.cache.join(format!("{key}.json")),
            json!({ "content": content }).to_string(),
        );
    }
}

/// What one (source, target) pair produced.
struct Step {
    source: usize,
    before: String,
    after: String,
    candidates: Candidates,
    dropped: Vec<usize>,
}

#[allow(clippy::too_many_lines)]
fn execute(
    args: &IngestArgs,
    root: &Path,
    vocab: &VaultVocabulary,
    profile: &VaultProfile,
    json: bool,
) -> anyhow::Result<ExitCode> {
    if args.targets.is_empty() && !args.discover {
        return refuse("name a --target or ask for --discover");
    }
    // The endpoint policy is Jev's: local by default, an explicit opt-in for the cloud.
    let env = |k: &str| std::env::var(k).ok();
    let mut endpoint = None;
    if !args.plan_only {
        let ep = args.endpoint.as_deref().unwrap_or_default();
        JevConfig::from_env(&env)
            .allows_endpoint(ep, &env)
            .map_err(|u| match u.reason.as_str() {
                "not-local" => Refused(format!(
                    "endpoint {ep} is not local; vault notes are private. Set \
                     ONTOMARK_JEV_ALLOW_CLOUD=1 to send them to it"
                )),
                _ => Refused(format!(
                    "endpoint {ep:?} is not an http(s) URL without credentials"
                )),
            })?;
        endpoint = Some(chat_url(ep));
    }

    // `vault propose` reads the knowledge vault only: a working page in the
    // manifest would be proposed as a new public page. Working notes are
    // private (ADR-OM-010), so they are refused rather than leaked.
    let kinds = vault_kinds(&args.vault);
    if kinds != [VaultKind::Knowledge] {
        return refuse(
            "ingest proposes to the public corpus and reads the knowledge vault only; \
             working notes are private and `vault propose` would publish them as new pages",
        );
    }
    let notes = load_notes(root, profile, &kinds)?;
    let model_profile = ingest_profile::build(vocab)?;
    let model = Model::build(&model_profile, notes.iter().map(|n| (n.id.clone(), &n.doc)));
    let options = Options::default();
    let ingest = Ingest::new(&model_profile, &model, options.clone());

    let ledger_path = args
        .ledger
        .clone()
        .unwrap_or_else(|| PathBuf::from(".ontomark/ingest/ledger.json"));
    let ledger_dir = ledger_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let mut ledger = Ledger::read_or_new(&ledger_path)?;

    let fingerprint = version_of(
        &notes
            .iter()
            .map(|n| format!("{}\0{}", n.id, n.doc.digest().hex()))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let model_name = args.model.clone().unwrap_or_default();
    let options_json = json!({
        "contract": CONTRACT_SOURCE,
        "instructions": version_of(INSTRUCTIONS_SOURCE),
        "options": serde_json::to_value(&options)?,
        "max_tokens": MAX_TOKENS,
        "discover": args.discover,
        "max_targets": args.max_targets,
        "vault": args.vault,
    });
    // A source is "unchanged" only against the same reading of it: model,
    // endpoint, options and corpus all part of the parser profile.
    let parser = format!(
        "ingest/{}",
        version_of(&format!(
            "{options_json}\0{model_name}\0{}\0{fingerprint}",
            endpoint.as_deref().unwrap_or("")
        ))
    );
    let sources = fetch_sources(args, &ledger, &parser)?;
    if sources.is_empty() {
        return refuse("no readable source");
    }

    // Targets, per source.
    let mut explicit: Vec<DocId> = Vec::new();
    for t in &args.targets {
        explicit.push(resolve_target(&ingest, &notes, t)?);
    }
    let mut per_source: Vec<Vec<DocId>> = Vec::new();
    for s in &sources {
        let mut t = explicit.clone();
        if args.discover {
            t.extend(discover(&ingest, &s.text.text, args.max_targets));
        }
        t.sort();
        t.dedup();
        per_source.push(t);
    }
    let targets: BTreeSet<&DocId> = per_source.iter().flatten().collect();
    let note_of = |id: &DocId| {
        notes
            .iter()
            .find(|n| &n.id == id)
            .expect("target is a note")
    };

    // The request id: sources, targets and options, not budget or paths.
    let request_id = version_of(
        &json!({
            "sources": sources.iter().map(|s| json!([s.item.id, s.fetched.version])).collect::<Vec<_>>(),
            "targets": targets.iter().map(|t| json!([t.as_str(), note_of(t).doc.digest().hex()])).collect::<Vec<_>>(),
            "model": model_name,
            "options": options_json,
        })
        .to_string(),
    )[..16]
        .to_owned();

    // Pairs, target-major, sources in id order.
    let mut pairs: Vec<(DocId, usize)> = Vec::new();
    for t in &targets {
        for (si, list) in per_source.iter().enumerate() {
            if list.contains(t) {
                pairs.push(((*t).clone(), si));
            }
        }
    }

    if args.plan_only {
        let mut planned = Vec::new();
        for (t, si) in &pairs {
            let n = note_of(t);
            let r = ingest.plan_source(t, &n.doc, &sources[*si].text);
            planned.push(match r {
                Ok(req) => json!({ "target": t.as_str(), "source": sources[*si].item.id,
                    "chunks": req.chunks.len(), "keys": req.keys.len() }),
                Err(e) => json!({ "target": t.as_str(), "source": sources[*si].item.id,
                    "error": e.to_string() }),
            });
        }
        let value = json!({ "request_id": request_id, "plan_only": true,
            "sources": sources.iter().map(|s| json!({"id": s.item.id, "version": s.fetched.version,
                "media_type": s.fetched.media_type, "change": s.change})).collect::<Vec<_>>(),
            "targets": targets.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
            "pairs": planned });
        crate::emit(json, &value, || {
            println!("request {request_id} (plan only; no model called, nothing written)");
            for s in &sources {
                println!(
                    "  source {} [{}] {}",
                    s.item.id,
                    s.change,
                    &s.fetched.version[..12]
                );
            }
            for p in &planned {
                println!(
                    "  {} <- {} ({} chunk(s))",
                    p["target"].as_str().unwrap_or(""),
                    p["source"].as_str().unwrap_or(""),
                    p["chunks"]
                );
            }
        })?;
        return Ok(ExitCode::SUCCESS);
    }

    let mut caller = Caller {
        url: endpoint.expect("set unless plan-only"),
        model: model_name.clone(),
        bearer: args
            .key_env
            .as_deref()
            .and_then(|v| std::env::var(v).ok())
            .filter(|v| !v.trim().is_empty()),
        cache: ledger_dir.join("cache"),
        max_calls: args.budget_calls,
        max_chars: args.budget_chars,
        calls: 0,
        chars: 0,
        hits: 0,
    };

    // Run every pair; a target's later sources are accepted against the page
    // as the earlier ones patched it, while requests stay planned against the
    // corpus page so cache keys do not depend on source order.
    let mut stop: Option<Stop> = None;
    let mut skipped: Vec<Value> = Vec::new();
    let mut finished: BTreeMap<usize, bool> = (0..sources.len()).map(|i| (i, true)).collect();
    let mut steps: BTreeMap<DocId, Vec<Step>> = BTreeMap::new();
    let mut finals: BTreeMap<DocId, Document> = BTreeMap::new();
    'pairs: for (t, si) in &pairs {
        if let Some(s) = &stop {
            skipped.push(skip_record(&sources[*si], t, s));
            finished.insert(*si, false);
            continue;
        }
        let n = note_of(t);
        let current = finals.get(t).cloned().unwrap_or_else(|| n.doc.clone());
        let src = &sources[*si];
        let base = match ingest.plan_source(t, &n.doc, &src.text) {
            Ok(r) => r,
            Err(e) => {
                skipped.push(
                    json!({ "source": src.item.id, "target": t.as_str(), "reason": e.to_string() }),
                );
                finished.insert(*si, false);
                continue;
            }
        };
        if base.chunks.len() > MAX_CHUNKS {
            skipped.push(json!({ "source": src.item.id, "target": t.as_str(),
                "reason": format!("too_many_chunks: {} > {MAX_CHUNKS}", base.chunks.len()) }));
            finished.insert(*si, false);
            continue;
        }
        let mut responses = Vec::new();
        let mut dropped = Vec::new();
        for chunk in &base.chunks {
            let body = request_body(&model_name, &base, chunk);
            let (content, cached) = match caller.answer(&body) {
                Ok(c) => c,
                Err(s) => {
                    skipped.push(skip_record(src, t, &s));
                    finished.insert(*si, false);
                    stop = Some(s);
                    continue 'pairs;
                }
            };
            match parse_answer(&content, chunk.index) {
                Some(r) => {
                    if !cached {
                        caller.remember(&body, &content);
                    }
                    responses.push(r);
                }
                None => dropped.push(chunk.index),
            }
        }
        let request = if current.digest() == n.doc.digest() {
            base
        } else {
            let again = ingest.plan_source(t, &current, &src.text)?;
            anyhow::ensure!(again.chunks == base.chunks, "chunks changed between plans");
            again
        };
        let candidates = ingest.accept_source(&current, &src.text, &request, &responses)?;
        let after = match &candidates.patch {
            Some(p) => {
                current
                    .apply(p)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .document
            }
            None => current.clone(),
        };
        steps.entry(t.clone()).or_default().push(Step {
            source: *si,
            before: current.digest().hex(),
            after: after.digest().hex(),
            candidates,
            dropped,
        });
        finals.insert(t.clone(), after);
    }

    // The bundle.
    let bundle = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!(".ontomark/ingest/{request_id}")));
    let manifest_dir = bundle.join("manifest");
    if manifest_dir.exists() {
        std::fs::remove_dir_all(&manifest_dir)?;
    }
    let mut pages: Vec<String> = Vec::new();
    let mut review: Vec<Value> = Vec::new();
    let mut target_records: Vec<Value> = Vec::new();
    for t in &targets {
        let n = note_of(t);
        let list = steps.get(*t).map_or(&[][..], Vec::as_slice);
        let changed = finals.get(*t).is_some_and(|d| d.source() != n.doc.source());
        if changed {
            let path = manifest_dir.join(format!("{}.md", t.as_str()));
            std::fs::create_dir_all(path.parent().expect("has a parent"))?;
            std::fs::write(&path, finals[*t].source())?;
            pages.push(t.as_str().to_owned());
        }
        let mut step_values = Vec::new();
        for s in list {
            let c = &s.candidates;
            for w in &c.withheld {
                if let WithholdReason::ConflictingValue { .. }
                | WithholdReason::CompetingValues { .. } = w.reason
                {
                    review.push(
                        json!({ "target": t.as_str(), "source": sources[s.source].item.id,
                        "key": w.key, "value": w.value, "reason": w.reason }),
                    );
                }
            }
            step_values.push(json!({
                "source": { "id": sources[s.source].item.id, "version": sources[s.source].fetched.version },
                "digest_before": s.before,
                "digest_after": s.after,
                "accepted": c.accepted,
                "withheld": c.withheld,
                "counts": c.counts,
                "diagnostics": c.diagnostics,
                "dropped_chunks": s.dropped,
            }));
        }
        target_records.push(json!({
            "id": t.as_str(),
            "vault": n.vault.role_name(),
            "label": ingest.vocabulary().terms().iter().find(|x| &x.document == *t).map(|x| x.label.clone()),
            "digest": n.doc.digest().hex(),
            "final_digest": finals.get(*t).map(|d| d.digest().hex()),
            "manifest": changed.then(|| format!("manifest/{}.md", t.as_str())),
            "steps": step_values,
        }));
    }
    let complete = skipped.is_empty() && stop.is_none();
    let evidence = json!({
        "request_id": request_id,
        "contract": CONTRACT_SOURCE,
        "model": model_name,
        "complete": complete,
        "sources": sources.iter().map(|s| json!({ "id": s.item.id, "version": s.fetched.version,
            "media_type": s.fetched.media_type })).collect::<Vec<_>>(),
        "targets": target_records,
        "review": review,
        "skipped": skipped,
        "provenance_keys": [],
    });
    std::fs::create_dir_all(&bundle)?;
    std::fs::write(
        bundle.join("evidence.json"),
        serde_json::to_string_pretty(&evidence)? + "\n",
    )?;

    // Record the sources that were read to the end.
    for (i, s) in sources.iter().enumerate() {
        if finished.get(&i).copied().unwrap_or(false) {
            ledger.record(&s.item, &s.fetched, &parser);
        }
    }
    std::fs::create_dir_all(&ledger_dir)?;
    ledger.write(&ledger_path)?;

    let manifest_dir = std::fs::canonicalize(&manifest_dir).unwrap_or(manifest_dir);
    let title = format!(
        "ingest: {} page(s) from {} source(s)",
        pages.len(),
        sources.len()
    );
    let hypothesis = format!(
        "External source evidence ({}); request {request_id}; quotes are in the bundle's evidence.json.",
        sources
            .iter()
            .map(|s| if s.item.id.starts_with("file:") {
                format!("local file {}", &s.fetched.version[..12])
            } else {
                s.item.id.clone()
            })
            .collect::<Vec<_>>()
            .join(", ")
    );
    let command = pages.first().map(|first| {
        format!(
            "vault propose {} --repo {} --diff {} --title {} --hypothesis {}",
            shell_quote(first),
            shell_quote(&root.display().to_string()),
            shell_quote(&manifest_dir.display().to_string()),
            shell_quote(&title),
            shell_quote(&hypothesis),
        )
    });
    let budget_exhausted = matches!(stop, Some(Stop::Budget(_)));
    let unavailable = match &stop {
        Some(Stop::Unavailable(m)) => Some(m.clone()),
        _ => None,
    };
    let summary = json!({
        "request_id": request_id,
        "bundle": bundle.display().to_string(),
        "complete": complete,
        "budget_exhausted": budget_exhausted,
        "model_unavailable": unavailable,
        "model_calls": caller.calls,
        "cache_hits": caller.hits,
        "chars_sent": caller.chars,
        "pages": pages,
        "review": evidence["review"].as_array().map_or(0, Vec::len),
        "skipped": evidence["skipped"],
        "sources": sources.iter().map(|s| json!({ "id": s.item.id, "version": s.fetched.version,
            "media_type": s.fetched.media_type, "change": s.change })).collect::<Vec<_>>(),
        "propose_command": command,
    });
    crate::emit(json, &summary, || {
        println!(
            "ingest {request_id}: {} page(s) proposed, {} model call(s), {} cached, {} for review{}",
            pages.len(),
            caller.calls,
            caller.hits,
            summary["review"],
            if budget_exhausted { " (budget exhausted; the rest was skipped)" } else { "" },
        );
        println!("bundle: {}", bundle.display());
        if let Some(m) = &unavailable {
            eprintln!("ontomark: the model endpoint failed: {m}");
        }
        match &command {
            Some(c) => println!("review evidence.json, then propose it yourself:\n{c}"),
            None => println!("nothing to propose"),
        }
    })?;
    Ok(if unavailable.is_some() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn skip_record(src: &Fetch, target: &DocId, stop: &Stop) -> Value {
    let reason = match stop {
        Stop::Budget(r) => (*r).to_owned(),
        Stop::Unavailable(m) => format!("model_unavailable: {m}"),
    };
    json!({ "source": src.item.id, "target": target.as_str(), "reason": reason })
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
