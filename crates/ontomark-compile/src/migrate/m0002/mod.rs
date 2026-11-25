//! Migration `m0002-vocabulary-v3`: a visionGraph vault from vocabulary
//! version 2 to version 3 (ADR-VG-010 to ADR-VG-013, `LOCKED-PLAN.md` Part B).
//!
//! What it does, by change class (the names the plan's classified diff uses):
//!
//! | class | pages | rule |
//! |---|---|---|
//! | `stamp` | every page with a `generated` block | `by` names the principal, `via` the instrument it directed (B8) |
//! | `verified` | pages with a `verified` entry | the same rewrite; the entry is otherwise untouched |
//! | `visibility` | working pages | `public: true` becomes `visibility: public`, everything else `shared`; `public` is dropped (B9) |
//! | `page_resource` | working pages without one | `urn:visionflow:page:<sha256 of the page id>` (B4) |
//! | `episode_status` | ledgers | `status: stable` becomes `approved` (B10) |
//! | `ledger_origin` | ledgers | `episode-url`, `episode-date` and a `rel: origin` source from the transcript |
//! | `brief` | topic aggregates | retyped `Brief` with `scope: topic`; sources become `derived-from` ledger links pinned `sha256-v1:` |
//! | `count_keys` | topic aggregates | the derivable `episodes`, `assertions` and `source` keys are dropped (B12) |
//! | `class_keys` | working Notes carrying a class `resource` | `resource` and `domain` are dropped |
//! | `supersedes` | merge winners | entries from the explicit input file [`INPUT_FILE`] (B11) |
//! | `alias` | merge winners | the loser's title is kept as an alias |
//! | `vocabulary`, `roster` | the two config files | version 3, the working kinds, the principal registry |
//!
//! # Contract
//!
//! * **Lossless.** Every edit is a guarded `ontomark-parse` patch: bytes outside
//!   the edited keys, unknown keys and key order are preserved, and the body is
//!   never touched. The config files are edited as text, comments kept.
//! * **Idempotent.** A migrated page transforms to [`Transform::Unchanged`].
//! * **Never guesses.** A page whose result cannot be derived is
//!   [`Transform::Refused`] with the reason, and left as it is. The plan
//!   validates the whole migrated tree and refuses to apply while anything
//!   would not validate.
//! * **Context from the tree.** [`VocabularyV3`] is a shell; [`Migration::prepare`]
//!   reads the roster, the ledgers, the transcripts and [`INPUT_FILE`] from the
//!   repository and returns the migration that runs. Ledger pins are the digest
//!   of the ledger *as migrated*, so they come out the same whether the ledger
//!   has been migrated yet or not.
//!
//! # Not done here, on purpose
//!
//! * Templater templates under `templates/` hold `<% %>` placeholders and are
//!   not parseable YAML: they are listed for a human.
//! * `contributors` starts empty (the key is simply absent), and `valid-from`
//!   is never backfilled (B5).
//! * The `Large Language Models` merge (B11) is a content decision: the page is
//!   listed when its reverse links disagree with its `episodes` count.
//! * A `verified` entry keeps its original (absent) review binding: adding a
//!   digest would certify content nobody reviewed.

mod config;
mod page;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{Kind, MigrateError, Migration, Transform};
use crate::input::principal::{did_body, npub_to_hex};

/// The principal every estate stamp resolves to (ADR-VG-012 §1, owner ruling
/// of 2026-11-23): the key that signs forum 31403 approvals.
pub const CANONICAL_PRINCIPAL: &str =
    "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";

/// The explicit input naming the merged losers (ADR-VG-010 §3, B11). The
/// loser IRIs and decision times exist only in git history, so an operator
/// extracts them, read-only, into this file; its digest is part of the plan.
///
/// ```yaml
/// supersessions:
///   - page: Decentralised Finance          # the live winner, a knowledge page id
///     resource: urn:ngm:class:decentralized-finance   # the loser's IRI
///     reason: merged                       # or renamed
///     at: 2026-09-22T12:41:39Z             # strict UTC, from the decision commit
///     by: did:nostr:b41654…                # optional, default the canonical principal
///     via: process:apply-decisions/1.0     # optional instrument
///     alias: Decentralized Finance         # optional, the loser's title
/// ```
pub const INPUT_FILE: &str = "ontology/m0002-supersessions.yaml";

pub(crate) const VOCABULARY: &str = "ontology/vocabulary.yaml";
pub(crate) const ROSTER: &str = "ontology/attribution.yaml";
const LEDGER_DIR: &str = "working/podcast-evidence/";

/// The `m0002-vocabulary-v3` migration (the registry's context-free shell).
#[derive(Debug, Clone, Copy, Default)]
pub struct VocabularyV3;

impl Migration for VocabularyV3 {
    fn id(&self) -> &'static str {
        "m0002-vocabulary-v3"
    }

    fn version(&self) -> u32 {
        1
    }

    fn description(&self) -> &'static str {
        "Upgrade a vault from vocabulary 2 to 3: principal and instrument stamps, working-memory kinds, visibility, page identity, derived sources"
    }

    fn kind(&self) -> Kind {
        Kind::Semantic
    }

    fn transform(&self, _page_rel_path: &str, _source: &str) -> Transform {
        Transform::Refused(
            "m0002 needs the repository's roster, ledgers and transcripts: run it through `ontomark migrate`"
                .to_owned(),
        )
    }

    fn prepare(&self, repo_root: &Path) -> Result<Option<Box<dyn Migration>>, MigrateError> {
        Ok(Some(Box::new(Prepared {
            ctx: Context::read(repo_root)?,
        })))
    }

    fn extra_paths(&self) -> Vec<String> {
        vec![VOCABULARY.to_owned(), ROSTER.to_owned()]
    }

    fn classify(&self, summary: &str) -> Vec<String> {
        classes_of(summary)
    }

    fn validates_result(&self) -> bool {
        true
    }
}

/// The change classes a summary names: it is a comma-separated tag list.
fn classes_of(summary: &str) -> Vec<String> {
    summary
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One entry of [`INPUT_FILE`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Supersession {
    pub page: String,
    pub resource: String,
    pub reason: String,
    pub at: String,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub via: Option<String>,
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct SupersessionFile {
    #[serde(default)]
    supersessions: Vec<Supersession>,
}

/// A ledger as the aggregates see it.
#[derive(Debug, Clone)]
pub(crate) struct LedgerInfo {
    /// `sha256-v1:` digest of the ledger as migrated.
    pub pin: String,
}

/// Everything a page rewrite reads besides the page.
#[derive(Debug, Default)]
pub(crate) struct Context {
    /// Hex keys that are the operator's: the canonical key, the human
    /// generators' keys and the registry's `also` keys.
    pub human_keys: BTreeSet<String>,
    /// Roster labels of human generators, as in `by: human:jjohare`.
    pub human_labels: BTreeSet<String>,
    /// Ledger id to what the aggregates need of it.
    pub ledgers: BTreeMap<String, LedgerInfo>,
    /// Wikilink target to the ledger ids whose bodies link it.
    pub reverse: BTreeMap<String, BTreeSet<String>>,
    /// Transcript slug to `(recording URL, date)`.
    pub transcripts: BTreeMap<String, (String, String)>,
    /// Winner page id to its supersession entries, input order.
    pub supersessions: BTreeMap<String, Vec<Supersession>>,
    /// Entries naming a winner that is not a page, for the input file's refusal.
    pub orphans: Vec<String>,
    /// Problems reading the input file.
    pub input_problem: Option<String>,
    /// Templater templates (`working/templates/*.md`, `knowledge/templates/*.md`),
    /// listed for a human and never rewritten.
    pub templates: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct RosterGenerator {
    #[serde(default)]
    did: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    npub: String,
}

#[derive(Debug, serde::Deserialize)]
struct RosterPrincipal {
    #[serde(default)]
    did: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    also: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RosterFile {
    #[serde(default)]
    generators: Vec<RosterGenerator>,
    #[serde(default)]
    principals: Vec<RosterPrincipal>,
}

fn read_text(path: &Path) -> Result<Option<String>, MigrateError> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(MigrateError::io(path, e)),
    }
}

fn sorted_markdown(dir: &Path) -> Result<Vec<std::path::PathBuf>, MigrateError> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    for entry in read {
        let path = entry.map_err(|e| MigrateError::io(dir, e))?.path();
        if path.extension().is_some_and(|e| e == "md") && path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

impl Context {
    /// Read the context from the repository. Deterministic for a given tree,
    /// and the same from an unmigrated or a part-migrated one.
    pub(crate) fn read(repo_root: &Path) -> Result<Self, MigrateError> {
        let mut ctx = Self::default();
        ctx.human_keys
            .insert(did_body(CANONICAL_PRINCIPAL).unwrap_or_default().to_owned());
        if let Some(text) = read_text(&repo_root.join(ROSTER))? {
            let roster: RosterFile = serde_norway::from_str(&text)
                .map_err(|e| MigrateError::Input(format!("{ROSTER}: {e}")))?;
            for g in roster.generators.iter().filter(|g| g.kind == "human") {
                if let Some(body) = did_body(&g.did) {
                    ctx.human_keys.insert(body.to_owned());
                }
                if let Ok(hex) = npub_to_hex(&g.npub) {
                    ctx.human_keys.insert(hex);
                }
                if !g.label.is_empty() {
                    ctx.human_labels.insert(g.label.clone());
                }
            }
            for p in roster.principals.iter().filter(|p| p.kind == "human") {
                for did in std::iter::once(&p.did).chain(&p.also) {
                    if let Some(body) = did_body(did) {
                        ctx.human_keys.insert(body.to_owned());
                    }
                }
            }
        }
        for vault in ["knowledge", "working"] {
            for path in sorted_markdown(&repo_root.join(vault).join("templates"))? {
                if let Some(name) = path.file_name() {
                    ctx.templates
                        .push(format!("{vault}/templates/{}", name.to_string_lossy()));
                }
            }
        }
        for path in sorted_markdown(&repo_root.join("transcripts"))? {
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(text) = read_text(&path)? {
                if let Some(found) = page::transcript_facts(&text) {
                    ctx.transcripts.insert(slug, found);
                }
            }
        }
        // Ledgers first as migrated, without pins (a ledger needs none), so the
        // aggregates can pin what the ledgers will be.
        for path in sorted_markdown(&repo_root.join(LEDGER_DIR))? {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let rel = format!("{LEDGER_DIR}{stem}.md");
            let id = format!("podcast-evidence/{stem}");
            let Some(text) = read_text(&path)? else {
                continue;
            };
            let migrated = match page::working(&ctx, &rel, &text) {
                Transform::Changed { source, .. } => source,
                _ => text,
            };
            let Some(pin) = page::pin_of(&migrated) else {
                continue;
            };
            let targets = page::body_targets(&migrated);
            for target in &targets {
                ctx.reverse
                    .entry(target.clone())
                    .or_default()
                    .insert(id.clone());
            }
            ctx.ledgers.insert(id, LedgerInfo { pin });
        }
        match read_text(&repo_root.join(INPUT_FILE))? {
            None => {}
            Some(text) => match serde_norway::from_str::<SupersessionFile>(&text) {
                Err(e) => ctx.input_problem = Some(format!("{INPUT_FILE}: {e}")),
                Ok(file) => {
                    for entry in file.supersessions {
                        let exists = repo_root
                            .join("knowledge")
                            .join(format!("{}.md", entry.page))
                            .is_file();
                        if exists {
                            ctx.supersessions
                                .entry(entry.page.clone())
                                .or_default()
                                .push(entry);
                        } else {
                            ctx.orphans
                                .push(format!("{} (loser {})", entry.page, entry.resource));
                        }
                    }
                }
            },
        }
        Ok(ctx)
    }
}

/// [`VocabularyV3`] bound to the context of one repository.
#[derive(Debug)]
struct Prepared {
    ctx: Context,
}

impl Migration for Prepared {
    fn id(&self) -> &str {
        VocabularyV3.id()
    }

    fn version(&self) -> u32 {
        VocabularyV3.version()
    }

    fn description(&self) -> &str {
        VocabularyV3.description()
    }

    fn kind(&self) -> Kind {
        Kind::Semantic
    }

    fn transform(&self, rel: &str, source: &str) -> Transform {
        if rel == VOCABULARY {
            return config::vocabulary(source);
        }
        if rel == ROSTER {
            return config::roster(source);
        }
        if rel == INPUT_FILE {
            return self.input_file();
        }
        if rel.contains("/templates/") {
            return Transform::Refused(
                "a Templater template (placeholders, not YAML): update its stamp, Episode and Draft Concept shapes by hand".to_owned(),
            );
        }
        if rel.starts_with("knowledge/") {
            return page::knowledge(&self.ctx, rel, source);
        }
        if rel.starts_with("working/") {
            return page::working(&self.ctx, rel, source);
        }
        Transform::Unchanged
    }

    fn extra_paths(&self) -> Vec<String> {
        let mut out = VocabularyV3.extra_paths();
        out.push(INPUT_FILE.to_owned());
        out.extend(self.ctx.templates.iter().cloned());
        out
    }

    fn classify(&self, summary: &str) -> Vec<String> {
        classes_of(summary)
    }

    fn validates_result(&self) -> bool {
        true
    }
}

impl Prepared {
    /// The input file is never rewritten; it is refused when it cannot be
    /// used, so the plan lists why.
    fn input_file(&self) -> Transform {
        if let Some(problem) = &self.ctx.input_problem {
            return Transform::Refused(format!("unreadable: {problem}"));
        }
        if !self.ctx.orphans.is_empty() {
            return Transform::Refused(format!(
                "{} supersession(s) name a winner that is not a knowledge page: {}",
                self.ctx.orphans.len(),
                self.ctx
                    .orphans
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        Transform::Unchanged
    }
}
