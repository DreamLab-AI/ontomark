//! `ontomark memory` and the lossless half of `ontomark edit`: the CLI face of
//! the one write engine (`ontomark-write`, ADR-VG-012 section 9).
//!
//! Every verb builds the engine from the repository, plans the write, prints
//! the plan, and (unless `--dry-run`) commits it atomically. A refusal writes
//! nothing and exits 2, with one JSON document under `--json`.

use std::path::Path;
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Args, Subcommand, ValueEnum};
use ontomark_compile::build::attribution::Roster;
use ontomark_compile::input::page::{Vault, VaultKind};
use ontomark_compile::input::vocabulary::Vocabulary;
use ontomark_compile::profile::Profile;
use ontomark_compile::write::{apply, create};
use ontomark_parse::{Document, Value};
use ontomark_write::{
    system, AdoptRequest, Change, CommitError, CompleteRequest, CreateRequest, DisputeInput,
    Engine, FsStore, Move, PatchRequest, Plan, PlannedWrite, RecordDisputesRequest, Refusal,
    Resolution, Role, SupersedeReason, TransitionRequest, Withdrawal, Writer,
};
use serde_json::json;

/// Who is writing, shared by every verb that stamps.
#[derive(Debug, Clone, Args)]
pub struct WriterArgs {
    /// The principal answerable for the write: `did:nostr:<64 hex>` (or a
    /// `human:npub…`, written back as its DID). Required for a version 3
    /// working-page write.
    #[arg(long, env = "ONTOMARK_PRINCIPAL", value_name = "DID")]
    pub by: Option<String>,
    /// The instrument that did the work: `agent:<name>/<version>` or
    /// `process:<name>/<version>`. Absent when the principal edits directly.
    /// An approval or confirmation never carries one.
    #[arg(long, env = "ONTOMARK_INSTRUMENT", value_name = "AGENT")]
    pub via: Option<String>,
    /// Override the clock (UTC, `YYYY-MM-DDTHH:MM:SSZ`). Debug builds only: the
    /// engine stamps both review clocks itself and ignores a client's values
    /// (ADR-VG-013 section 7), so a release binary has no way to backdate one.
    #[cfg(debug_assertions)]
    #[arg(long, hide = true, value_name = "TIMESTAMP")]
    pub now: Option<String>,
}

impl WriterArgs {
    fn writer(&self) -> anyhow::Result<Writer> {
        let by = self.by.clone().context(
            "this write is stamped: name the principal with --by (or ONTOMARK_PRINCIPAL)",
        )?;
        Ok(Writer {
            by,
            via: self.via.clone(),
        })
    }

    #[cfg(debug_assertions)]
    fn at(&self) -> String {
        self.now.clone().unwrap_or_else(system::now)
    }

    #[cfg(not(debug_assertions))]
    #[allow(clippy::unused_self)]
    fn at(&self) -> String {
        system::now()
    }
}

/// `ontomark memory`: working-memory writes through the engine.
#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Create a working page: minted identity, first stamp, visibility, draft.
    Create(CreateArgs),
    /// Give a working page (a daily note, say) the identity it lacks.
    Adopt(IdArgs),
    /// draft to approved. A draft holding disputes supersedes them, atomically.
    Approve(ApproveArgs),
    /// draft to invalidated.
    Reject(WithdrawArgs),
    /// approved to approved: a review that finds the page still good.
    Confirm(IdArgs),
    /// approved to draft, with disputes, a reason, or both.
    SendBack(SendBackArgs),
    /// approved to invalidated.
    Withdraw(WithdrawArgs),
    /// approved to archived.
    Archive(ArchiveArgs),
    /// Replace the disputes a draft holds (none clears them).
    Dispute(DisputeArgs),
    /// Finish an approve-then-supersede that was interrupted.
    Complete(IdArgs),
    /// Print the deterministic `page_resource` of a working page id, the one
    /// m0002 mints and a journal is adopted with: a template or hook asks the
    /// binary instead of hashing anything itself. Needs no vault.
    Resource(ResourceArgs),
}

/// A page id whose deterministic `page_resource` is wanted.
#[derive(Debug, Args)]
pub struct ResourceArgs {
    /// The working page id: its path without `.md`, for a journal
    /// `journals/YYYY-MM-DD`.
    pub id: String,
}

/// The page every verb acts on.
#[derive(Debug, Args)]
pub struct IdArgs {
    /// The working page id (its path without `.md`).
    pub id: String,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// `memory create`.
#[derive(Debug, Args)]
pub struct CreateArgs {
    /// The new page's id: its path under `working/` without `.md`.
    pub id: String,
    /// A working type of the vocabulary, for example `Belief`.
    #[arg(long = "type", value_name = "TYPE")]
    pub page_type: String,
    /// `key=value` (YAML) for each key of the page, in order.
    #[arg(long = "set", value_name = "K=V")]
    pub sets: Vec<String>,
    /// The body text.
    #[arg(long, conflicts_with = "body_file")]
    pub body: Option<String>,
    /// Read the body from a file.
    #[arg(long, value_name = "FILE")]
    pub body_file: Option<std::path::PathBuf>,
    /// `shared` (default) or `public`.
    #[arg(long)]
    pub visibility: Option<String>,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// `memory approve`.
#[derive(Debug, Args)]
pub struct ApproveArgs {
    /// The draft's id.
    pub id: String,
    /// One per dispute: `PAGE=world-changed` or `PAGE=corrected`, optionally
    /// `@YYYY-MM-DDTHH:MM:SSZ` after `world-changed` for when it stopped
    /// being true.
    #[arg(long = "resolve", value_name = "PAGE=REASON")]
    pub resolve: Vec<String>,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// `memory reject` and `memory withdraw`.
#[derive(Debug, Args)]
pub struct WithdrawArgs {
    /// The page id.
    pub id: String,
    /// Why: `corrected` (never true) or `withdrawn` (should not have been recorded).
    #[arg(long, value_enum)]
    pub reason: ReasonArg,
    /// Optional `status-reason` text.
    #[arg(long)]
    pub note: Option<String>,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// The two withdrawal reasons.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReasonArg {
    /// It was never true.
    Corrected,
    /// It should not have been recorded.
    Withdrawn,
}

/// `memory send-back`.
#[derive(Debug, Args)]
pub struct SendBackArgs {
    /// The approved page's id.
    pub id: String,
    /// `PAGE` or `PAGE=reason text`: an approved page this one now contradicts.
    #[arg(long = "against", value_name = "PAGE[=REASON]")]
    pub against: Vec<String>,
    /// Why it left approved.
    #[arg(long)]
    pub status_reason: Option<String>,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// `memory archive`.
#[derive(Debug, Args)]
pub struct ArchiveArgs {
    /// The approved page's id.
    pub id: String,
    /// When it stopped being relevant, `YYYY-MM-DDTHH:MM:SSZ`.
    #[arg(long)]
    pub valid_to: Option<String>,
    /// Optional `status-reason` text.
    #[arg(long)]
    pub note: Option<String>,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// `memory dispute`.
#[derive(Debug, Args)]
pub struct DisputeArgs {
    /// The draft's id.
    pub id: String,
    /// `PAGE` or `PAGE=reason text`; give none to clear the disputes.
    #[arg(long = "against", value_name = "PAGE[=REASON]")]
    pub against: Vec<String>,
    #[command(flatten)]
    pub writer: WriterArgs,
    /// Plan and report; write nothing.
    #[arg(long)]
    pub dry_run: bool,
}

/// Parse a CLI value as YAML with the lossless parser, keeping lexical forms
/// (`0.70` stays `0.70`). Anything that does not parse is a plain string.
#[must_use]
pub fn parse_value(raw: &str) -> Value {
    if raw.contains('\n') {
        return Value::string(raw);
    }
    let doc = Document::parse(format!("---\nk: {raw}\n---\n"));
    match doc.value("k") {
        Some(Value::Null) | None => Value::string(raw),
        Some(v)
            if doc
                .frontmatter()
                .is_some_and(ontomark_parse::Frontmatter::is_parsed) =>
        {
            v.clone()
        }
        Some(_) => Value::string(raw),
    }
}

fn split_set(arg: &str) -> anyhow::Result<(String, bool, Value)> {
    let (raw_key, value) = arg
        .split_once('=')
        .with_context(|| format!("`{arg}` is not `key=value` or `key+=value`"))?;
    let (key, append) = raw_key
        .strip_suffix('+')
        .map_or((raw_key, false), |k| (k, true));
    Ok((key.trim().to_owned(), append, parse_value(value)))
}

/// The engine and its file store for a repository.
fn build(root: &Path, profile: &Profile, vocab: &Vocabulary) -> anyhow::Result<(Engine, FsStore)> {
    let mut working: Option<Vault> = None;
    let mut knowledge: Option<Vault> = None;
    for kind in [VaultKind::Knowledge, VaultKind::Working] {
        match crate::load(root, profile, &[kind]) {
            Ok(mut v) => {
                let v = v.remove(0);
                match kind {
                    VaultKind::Knowledge => knowledge = Some(v),
                    VaultKind::Working => working = Some(v),
                }
            }
            Err(e)
                if e.downcast_ref::<ontomark_compile::profile::MissingVault>()
                    .is_some() => {}
            Err(e) => return Err(e),
        }
    }
    let roster = vocab
        .is_v3()
        .then(|| Roster::discover(root))
        .filter(|r| !(r.is_empty() && r.principals().is_empty()));
    let store = FsStore::new(
        working.as_ref().map(|v| v.root.as_path()),
        knowledge.as_ref().map(|v| v.root.as_path()),
    )?;
    let working = working.unwrap_or_else(|| Vault {
        root: root.join("working"),
        kind: VaultKind::Working,
        pages: Vec::new(),
        journals: Vec::new(),
        skipped: Vec::new(),
    });
    Ok((
        Engine::new(profile.clone(), vocab.clone(), roster, working, knowledge),
        store,
    ))
}

/// Patch one page through the engine (the lossless half of `ontomark edit`).
///
/// `sets` and `unsets` are the CLI's `--set` and `--unset` arguments. `base`
/// is the page text the caller's `--expect` guard counted over, and
/// `guard_blocks` the number of keys that guard counted: the engine plans
/// against exactly that text (a stale base is refused), and a plan that
/// changes a different number of requested keys than the guard saw is
/// refused rather than written, because the two parsers then disagree.
/// Returns the plan; commits it unless `dry_run`.
///
/// # Errors
/// A refusal or a write failure, as an `anyhow` error (exit 1, as `edit` always
/// reported its failures).
#[allow(clippy::too_many_arguments)]
pub fn patch_page(
    root: &Path,
    profile: &Profile,
    vocab: &Vocabulary,
    kind: VaultKind,
    id: &str,
    sets: &[String],
    unsets: &[String],
    writer: &WriterArgs,
    base: &str,
    guard_blocks: usize,
    dry_run: bool,
) -> anyhow::Result<Plan> {
    let mut changes = Vec::new();
    for s in sets {
        let (key, append, value) = split_set(s)?;
        changes.push(if append {
            Change::Append { key, value }
        } else {
            Change::Set { key, value }
        });
    }
    changes.extend(unsets.iter().map(|k| Change::Unset { key: k.clone() }));
    let (engine, mut store) = build(root, profile, vocab)?;
    let plan = engine
        .patch(&PatchRequest {
            role: if kind == VaultKind::Working {
                Role::Working
            } else {
                Role::Knowledge
            },
            id: id.to_owned(),
            changes,
            body: Vec::new(),
            expect: Some(ontomark_parse::SourceDigest::of(base)),
            writer: writer.by.as_ref().map(|by| Writer {
                by: by.clone(),
                via: writer.via.clone(),
            }),
            now: Some(writer.at()),
        })
        .map_err(|r| anyhow::anyhow!("refused: {r}"))?;
    anyhow::ensure!(
        plan.requested_keys.len() == guard_blocks,
        "refused: the write engine would change {} key(s) ({}) but the --expect guard counted {guard_blocks}; \
         the two readings of the page disagree, so nothing was written",
        plan.requested_keys.len(),
        plan.requested_keys.join(", ")
    );
    if !dry_run {
        plan.commit(&mut store)?;
    }
    Ok(plan)
}

/// Land one already-validated page write through the engine's store: the same
/// temp-file-and-rename (or exclusive create) and root confinement as every
/// other write. `before` is the text the caller validated against (`None` for
/// a creation).
fn commit_one(
    vault_root: &Path,
    target: &Path,
    id: &str,
    before: Option<&str>,
    text: &str,
) -> Result<(), CommitError> {
    let rel = target
        .strip_prefix(vault_root)
        .map_or_else(|_| target.to_path_buf(), Path::to_path_buf);
    let mut store = FsStore::new(None, Some(vault_root)).map_err(|source| CommitError::Read {
        path: rel.clone(),
        source,
    })?;
    let plan = Plan {
        operation: "write",
        writes: vec![PlannedWrite {
            vault: VaultKind::Knowledge,
            id: id.to_owned(),
            rel_path: rel,
            before_text: before.map(str::to_owned),
            text: text.to_owned(),
            receipt: None,
        }],
        requested_keys: Vec::new(),
        ignored_keys: Vec::new(),
        events: Vec::new(),
    };
    plan.commit(&mut store).map(|_| ())
}

/// `create`'s final write, through the engine's store. Errors are the ones
/// `create::write` returned, so the handler's refusal mapping is unchanged.
///
/// # Errors
/// [`create::CreateError::Exists`] when the page appeared meanwhile; otherwise
/// the I/O failure.
pub fn write_created(outcome: &create::Outcome, vault_root: &Path) -> anyhow::Result<()> {
    commit_one(
        vault_root,
        &outcome.target,
        &outcome.page,
        None,
        &outcome.rendered,
    )
    .map_err(|e| match &e {
        CommitError::Exists(_) => {
            anyhow::Error::new(create::CreateError::Exists(outcome.page.clone()))
        }
        CommitError::Write { source, .. } if source.kind() == std::io::ErrorKind::AlreadyExists => {
            anyhow::Error::new(create::CreateError::Exists(outcome.page.clone()))
        }
        _ => anyhow::Error::new(e).context(format!("writing {}", outcome.target.display())),
    })
}

/// `apply`'s final write, through the engine's store. A page that changed
/// since [`apply::prepare`] read it is refused as [`apply::ApplyError::Stale`].
///
/// # Errors
/// [`apply::ApplyError::Stale`], or the I/O failure.
pub fn write_applied(outcome: &apply::Outcome, vault_root: &Path) -> anyhow::Result<()> {
    commit_one(
        vault_root,
        &outcome.target,
        &outcome.page,
        Some(&outcome.original),
        &outcome.rendered,
    )
    .map_err(|e| match &e {
        CommitError::Stale(_) => anyhow::Error::new(apply::ApplyError::Stale {
            page: outcome.page.clone(),
            detail: "the file changed while the apply was being prepared".into(),
        }),
        _ => anyhow::Error::new(e).context(format!("writing {}", outcome.target.display())),
    })
}

fn finish(
    json_out: bool,
    operation: &str,
    result: Result<Plan, Refusal>,
    store: &mut FsStore,
    dry_run: bool,
) -> anyhow::Result<ExitCode> {
    let plan = match result {
        Ok(plan) => plan,
        Err(r) => {
            let value = json!({
                "written": false,
                "operation": operation,
                "code": r.code,
                "message": r.message,
                "issues": r.issues,
            });
            crate::emit(json_out, &value, || {})?;
            eprintln!("ontomark: refused: {r}");
            return Ok(ExitCode::from(2));
        }
    };
    if !dry_run {
        plan.commit(store)?;
    }
    let mut value = serde_json::to_value(&plan)?;
    value["written"] = json!(!dry_run && !plan.is_noop());
    value["dry_run"] = json!(dry_run);
    crate::emit(json_out, &value, || {
        if plan.is_noop() {
            println!("{operation}: nothing to change");
        }
        for w in &plan.writes {
            println!(
                "{} {}{}",
                if w.before_text.is_none() {
                    "create"
                } else {
                    "update"
                },
                w.rel_path.display(),
                if dry_run { " (dry run)" } else { "" }
            );
        }
        for k in &plan.ignored_keys {
            println!("  ignored client value for `{k}` (the engine stamps it)");
        }
        for e in &plan.events {
            println!("  review item {:?}: {}", e.kind, e.draft);
        }
    })?;
    Ok(ExitCode::SUCCESS)
}

fn dispute_inputs(against: &[String]) -> Vec<DisputeInput> {
    against
        .iter()
        .map(|a| match a.split_once('=') {
            Some((page, reason)) => DisputeInput {
                page: page.to_owned(),
                reason: Some(reason.to_owned()),
            },
            None => DisputeInput {
                page: a.clone(),
                reason: None,
            },
        })
        .collect()
}

fn resolutions(resolve: &[String]) -> anyhow::Result<Vec<Resolution>> {
    resolve
        .iter()
        .map(|r| {
            let (page, rest) = r
                .rsplit_once('=')
                .with_context(|| format!("`{r}` is not PAGE=world-changed or PAGE=corrected"))?;
            let (reason, valid_to) = match rest.split_once('@') {
                Some((reason, at)) => (reason, Some(at.to_owned())),
                None => (rest, None),
            };
            let reason = match reason {
                "world-changed" => SupersedeReason::WorldChanged,
                "corrected" => SupersedeReason::Corrected,
                other => anyhow::bail!("`{other}` is not world-changed or corrected"),
            };
            Ok(Resolution {
                page: page.to_owned(),
                reason,
                valid_to,
            })
        })
        .collect()
}

fn withdrawal(r: ReasonArg) -> Withdrawal {
    match r {
        ReasonArg::Corrected => Withdrawal::Corrected,
        ReasonArg::Withdrawn => Withdrawal::Withdrawn,
    }
}

/// Run one `ontomark memory` verb.
///
/// # Errors
/// A malformed argument or a write failure. A refusal is not an error: it is
/// reported and exits 2.
#[allow(clippy::too_many_lines)] // One arm per verb.
pub fn run(
    command: &MemoryCommand,
    root: &Path,
    profile: &Profile,
    vocab: &Vocabulary,
    json_out: bool,
) -> anyhow::Result<ExitCode> {
    if let MemoryCommand::Resource(a) = command {
        let id = a.id.trim().trim_end_matches(".md");
        anyhow::ensure!(!id.is_empty(), "a page id is required");
        let urn = ontomark_compile::check::v3::state::derived_page_resource(id);
        if json_out {
            println!("{}", serde_json::json!({ "id": id, "page_resource": urn }));
        } else {
            println!("{urn}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    let (engine, mut store) = build(root, profile, vocab)?;
    let transition =
        |id: &str, mv: Move, w: &WriterArgs| -> anyhow::Result<Result<Plan, Refusal>> {
            Ok(engine.transition(&TransitionRequest {
                id: id.to_owned(),
                expect: None,
                mv,
                writer: w.writer()?,
                now: w.at(),
            }))
        };
    match command {
        MemoryCommand::Create(a) => {
            let mut fields = Vec::new();
            for s in &a.sets {
                let (key, append, value) = split_set(s)?;
                anyhow::ensure!(!append, "`{key}+=`: a new page has nothing to append to");
                fields.push((key, value));
            }
            let body = match (&a.body, &a.body_file) {
                (Some(b), _) => b.clone(),
                (None, Some(f)) => std::fs::read_to_string(f)
                    .with_context(|| format!("reading {}", f.display()))?,
                (None, None) => String::new(),
            };
            let result = system::entropy()
                .map_err(|e| anyhow::anyhow!("no randomness for the page identity: {e}"))
                .map(|entropy| {
                    a.writer.writer().map(|writer| {
                        engine.create(&CreateRequest {
                            id: a.id.clone(),
                            page_type: a.page_type.clone(),
                            fields,
                            body,
                            visibility: a.visibility.clone(),
                            writer,
                            now: a.writer.at(),
                            entropy,
                        })
                    })
                })??;
            finish(json_out, "create", result, &mut store, a.dry_run)
        }
        MemoryCommand::Adopt(a) => {
            let entropy = system::entropy()
                .map_err(|e| anyhow::anyhow!("no randomness for the page identity: {e}"))?;
            let result = engine.adopt(&AdoptRequest {
                id: a.id.clone(),
                expect: None,
                writer: a.writer.writer()?,
                now: a.writer.at(),
                entropy,
            });
            finish(json_out, "adopt", result, &mut store, a.dry_run)
        }
        MemoryCommand::Approve(a) => {
            let result = transition(
                &a.id,
                Move::Approve {
                    resolutions: resolutions(&a.resolve)?,
                },
                &a.writer,
            )?;
            finish(json_out, "approve", result, &mut store, a.dry_run)
        }
        MemoryCommand::Reject(a) => {
            let mv = Move::Reject {
                reason: withdrawal(a.reason),
                note: a.note.clone(),
            };
            let result = transition(&a.id, mv, &a.writer)?;
            finish(json_out, "reject", result, &mut store, a.dry_run)
        }
        MemoryCommand::Withdraw(a) => {
            let mv = Move::Withdraw {
                reason: withdrawal(a.reason),
                note: a.note.clone(),
            };
            let result = transition(&a.id, mv, &a.writer)?;
            finish(json_out, "withdraw", result, &mut store, a.dry_run)
        }
        MemoryCommand::Confirm(a) => {
            let result = transition(&a.id, Move::Confirm, &a.writer)?;
            finish(json_out, "confirm", result, &mut store, a.dry_run)
        }
        MemoryCommand::SendBack(a) => {
            let mv = Move::SendBack {
                disputes: dispute_inputs(&a.against),
                status_reason: a.status_reason.clone(),
            };
            let result = transition(&a.id, mv, &a.writer)?;
            finish(json_out, "send_back", result, &mut store, a.dry_run)
        }
        MemoryCommand::Archive(a) => {
            let mv = Move::Archive {
                valid_to: a.valid_to.clone(),
                note: a.note.clone(),
            };
            let result = transition(&a.id, mv, &a.writer)?;
            finish(json_out, "archive", result, &mut store, a.dry_run)
        }
        MemoryCommand::Dispute(a) => {
            let result = engine.record_disputes(&RecordDisputesRequest {
                id: a.id.clone(),
                expect: None,
                disputes: dispute_inputs(&a.against),
                writer: a.writer.writer()?,
                now: a.writer.at(),
            });
            finish(json_out, "record_disputes", result, &mut store, a.dry_run)
        }
        MemoryCommand::Complete(a) => {
            let result = engine.complete_supersession(&CompleteRequest {
                id: a.id.clone(),
                writer: a.writer.writer()?,
                now: a.writer.at(),
            });
            finish(
                json_out,
                "complete_supersession",
                result,
                &mut store,
                a.dry_run,
            )
        }
        // Answered above, before any vault is read.
        MemoryCommand::Resource(_) => Ok(ExitCode::SUCCESS),
    }
}
