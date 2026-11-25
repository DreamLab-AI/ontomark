//! `m0002-vocabulary-v3` on a synthetic vocabulary 2 repository: the classified
//! diff, losslessness, idempotence, refusal, the unmigratable list and restart.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ontomark_compile::input::page::VaultKind;
use ontomark_compile::input::principal::npub_to_hex;
use ontomark_compile::migrate::{
    self, apply_with, plan, rollback, ApplyOptions, Disposition, MigrateError, Plan, PlanStatus,
};
use ontomark_compile::profile::Profile;

const BOTH: [VaultKind; 2] = [VaultKind::Knowledge, VaultKind::Working];
const B4: &str = "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
const FUNGIBLE_NPUB: &str = "npub1kst9gqtldpgtzwzh6xwc4c8rlz83xetqpj4sxg0gzqwfayng9aaqlz0mmn";

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml")).unwrap()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn ledger(title: &str, topic_links: &str) -> String {
    format!(
        "---\ntype: Episode\ntitle: {title}\npublic: false\nstatus: stable\ngenerated:\n  by: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:58.55607396Z\n---\n# {title}\n\n- A claim. {topic_links}\n  <!-- assertion-fp: 32ebb74ef2751e32 -->\n"
    )
}

const TRANSCRIPT: &str =
    "ingest-status:: processed\n# T\n\n- **Date**: 2026-03-10\n- **Duration**: 21:05\n- **YouTube**: https://www.youtube.com/watch?v=abc\n";

const KNOWLEDGE_BODY: &str = "Body text, kept byte for byte.\n";

/// A vocabulary 2 repository that migrates cleanly.
fn seeded() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    write(
        &root,
        "ontology/vocabulary.yaml",
        include_str!("m0002/vocabulary-v2.yaml"),
    );
    write(
        &root,
        "ontology/attribution.yaml",
        include_str!("m0002/attribution-v2.yaml"),
    );
    write(
        &root,
        "knowledge/Alpha.md",
        &format!("---\ntype: Class\ntitle: Alpha\nresource: urn:ngm:class:alpha\npublic: true\nstatus: draft\ncustom-key: kept\ngenerated:\n  by: agent:enrichment-swarm/6\n  process: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:39Z\n---\n{KNOWLEDGE_BODY}"),
    );
    write(
        &root,
        "knowledge/Fungible Token.md",
        &format!("---\ntype: Class\ntitle: Fungible Token\nresource: urn:ngm:class:fungible-token\npublic: true\nstatus: stable\ngenerated:\n  by: human:{FUNGIBLE_NPUB}\n  at: 2026-09-22T12:41:39Z\nverified:\n- by: human:{FUNGIBLE_NPUB}\n  at: 2026-10-05T18:59:14.000Z\n---\n{KNOWLEDGE_BODY}"),
    );
    write(&root, "transcripts/slug-a.md", TRANSCRIPT);
    write(&root, "transcripts/slug-b.md", TRANSCRIPT);
    write(
        &root,
        "working/podcast-evidence/slug-a.md",
        &ledger("Ledger A", "[[Topic X]]"),
    );
    write(
        &root,
        "working/podcast-evidence/slug-b.md",
        &ledger("Ledger B", "[[Topic X]] [[Other]]"),
    );
    write(
        &root,
        "working/Topic X.md",
        "---\ntype: Episode\ntitle: Topic X\npublic: false\nstatus: stable\ngenerated:\n  by: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:58.55607396Z\nsources:\n- id: source\n  resource: AI Daily Brief (promotion stage)\ntopic: Topic X\nepisodes: 2\nassertions: 4\npromotion-status: candidate_rejected\nunknown-key: [a, b]\n---\n# Topic X\n\nProse.\n",
    );
    write(
        &root,
        "working/Public Note.md",
        "---\ntype: Note\ntitle: Public Note\npublic: true\nstatus: stable\ngenerated:\n  by: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:58.55607396Z\nresource: urn:visionflow:owl:class:public-note\npage_resource: urn:visionflow:page:6b99a0cb36f6b695a8fdbf2d461aeedd79f5108ac6a6070c4046aaa204f38def\ndomain: infrastructure\n---\nNote body.\n",
    );
    write(
        &root,
        "working/journals/2026-10-12.md",
        "---\ntype: Journal\ntitle: 2026-10-12\npublic: false\nstatus: draft\ngenerated:\n  by: human:jjohare\n  at: 2026-10-12T09:00:00Z\n---\n- Did a thing\n",
    );
    (dir, root)
}

fn plan_of(root: &Path) -> Plan {
    let m = migrate::lookup("m0002-vocabulary-v3").unwrap();
    plan(root, &BOTH, m.as_ref(), &profile()).unwrap()
}

fn run_apply(
    root: &Path,
    plan: &Plan,
    opts: &ApplyOptions,
) -> Result<migrate::ApplyReport, MigrateError> {
    let m = migrate::lookup("m0002-vocabulary-v3").unwrap();
    apply_with(root, plan, m.as_ref(), opts)
}

fn accept() -> ApplyOptions {
    ApplyOptions {
        accept_semantic: true,
        ..ApplyOptions::default()
    }
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".ontomark") {
                continue;
            }
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(base).unwrap().to_string_lossy().into_owned(),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn text(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn the_key_in_the_fungible_token_stamp_is_the_canonical_principal() {
    assert_eq!(
        format!("did:nostr:{}", npub_to_hex(FUNGIBLE_NPUB).unwrap()),
        B4
    );
}

#[test]
fn the_plan_is_a_classified_diff_that_validates() {
    let (_dir, root) = seeded();
    let before = tree(&root);
    let plan = plan_of(&root);
    assert_eq!(tree(&root), before, "a plan writes nothing");
    let refused: Vec<_> = plan
        .pages
        .iter()
        .filter(|p| p.disposition == Disposition::Refused)
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(plan.status, PlanStatus::Ready, "{:?}", plan.reasons);
    // Pages and the two config files change; nothing is refused here.
    assert_eq!(plan.refused, 0, "{:?}", plan.pages);
    for (class, n) in [
        ("stamp", 7),
        ("verified", 1),
        ("visibility", 5),
        ("page_resource", 4),
        ("episode_status", 2),
        ("ledger_origin", 2),
        ("brief", 1),
        ("count_keys", 1),
        ("class_keys", 1),
        ("vocabulary", 1),
        ("roster", 1),
    ] {
        assert_eq!(
            plan.classes.get(class),
            Some(&n),
            "{class}: {:?}",
            plan.classes
        );
    }
    let report = plan.report();
    assert!(report.contains("classified diff"), "{report}");
}

#[test]
fn applying_migrates_losslessly_and_a_second_plan_is_empty() {
    let (_dir, root) = seeded();
    let plan = plan_of(&root);
    let report = run_apply(&root, &plan, &accept()).unwrap();
    assert!(report.complete, "{:?}", report.conflicts);

    // Stamps: the principal answers, the instrument is named, `at` and the
    // unknown `process` key are kept in place.
    let alpha = text(&root, "knowledge/Alpha.md");
    assert!(
        alpha.contains(&format!("generated:\n  by: {B4}\n  via: agent:enrichment-swarm/6\n  process: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:39Z\n")),
        "{alpha}"
    );
    assert!(alpha.contains("custom-key: kept\n"), "unknown keys survive");
    assert!(
        alpha.ends_with(&format!("---\n{KNOWLEDGE_BODY}")),
        "the body is untouched"
    );

    // Fungible Token: both entries re-encoded, the entry otherwise as it was.
    let ft = text(&root, "knowledge/Fungible Token.md");
    assert!(
        ft.contains(&format!(
            "verified:\n- by: {B4}\n  at: 2026-10-05T18:59:14.000Z\n"
        )),
        "{ft}"
    );
    assert!(!ft.contains("human:npub"));

    // Working: journals, visibility, identity.
    let journal = text(&root, "working/journals/2026-10-12.md");
    assert!(
        journal.contains(&format!("generated:\n  by: {B4}\n")),
        "{journal}"
    );
    assert!(journal.contains("visibility: shared\n") && !journal.contains("public:"));
    assert!(journal.contains("page_resource: urn:visionflow:page:"));
    let note = text(&root, "working/Public Note.md");
    assert!(note.contains("visibility: public\n") && !note.contains("\npublic:"));
    assert!(!note.contains("resource: urn:visionflow:owl") && !note.contains("domain:"));
    assert!(
        note.contains("page_resource: urn:visionflow:page:6b99a0cb"),
        "an existing page_resource is kept"
    );

    // A ledger: approved, its origin from the transcript.
    let a = text(&root, "working/podcast-evidence/slug-a.md");
    assert!(a.contains("status: approved\n"));
    assert!(a.contains("episode-url: https://www.youtube.com/watch?v=abc\n"));
    assert!(
        a.contains("episode-date: '2026-03-10'\n") || a.contains("episode-date: 2026-03-10\n"),
        "{a}"
    );
    assert!(a.contains("rel: origin"), "{a}");
    assert!(
        a.contains("<!-- assertion-fp: 32ebb74ef2751e32 -->"),
        "the body is untouched"
    );

    // The aggregate: a Brief over its two ledgers, pinned; counts dropped.
    let t = text(&root, "working/Topic X.md");
    assert!(
        t.contains("type: Brief\n") && t.contains("scope: topic\n"),
        "{t}"
    );
    assert!(
        t.contains("resource: '[[podcast-evidence/slug-a]]'")
            || t.contains("resource: \"[[podcast-evidence/slug-a]]\""),
        "{t}"
    );
    assert!(
        t.contains("rel: derived-from") && t.contains("locator: sha256-v1:"),
        "{t}"
    );
    assert!(
        !t.contains("episodes:") && !t.contains("assertions:") && !t.contains("AI Daily Brief"),
        "{t}"
    );
    assert!(
        t.contains("unknown-key: [a, b]\n"),
        "an unknown key keeps its form: {t}"
    );
    assert!(t.ends_with("# Topic X\n\nProse.\n"));

    // Config.
    let vocab = text(&root, "ontology/vocabulary.yaml");
    assert!(vocab.contains("\nversion: 3\n") && vocab.contains("context/v4.jsonld"));
    assert!(
        vocab.contains("# v2 (2026-11-04, ADR-VG-009)"),
        "comments survive"
    );
    let roster = text(&root, "ontology/attribution.yaml");
    assert!(
        roster.contains("principals:") && roster.contains("also:"),
        "{roster}"
    );
    assert!(
        roster.matches(&format!("principal: {B4}")).count() == 2,
        "{roster}"
    );

    // Idempotent: a second plan has nothing to do and refuses nothing.
    let again = plan_of(&root);
    assert_eq!(again.status, PlanStatus::Ready, "{:?}", again.reasons);
    assert_eq!(again.changes, 0, "{:?}", again.pages);
    assert_eq!(again.refused, 0, "{:?}", again.pages);
}

#[test]
fn rollback_restores_every_original_byte() {
    let (_dir, root) = seeded();
    let before = tree(&root);
    let plan = plan_of(&root);
    let report = run_apply(&root, &plan, &accept()).unwrap();
    assert_ne!(tree(&root), before);
    let restored = rollback(&root, &report.journal).unwrap();
    assert!(restored.conflicts.is_empty());
    assert_eq!(tree(&root), before);
}

#[test]
fn an_interrupted_run_finishes_from_a_part_migrated_tree() {
    let (_dir, root) = seeded();
    let plan = plan_of(&root);
    let opts = ApplyOptions {
        accept_semantic: true,
        stop_after: Some(3),
        ..ApplyOptions::default()
    };
    assert!(matches!(
        run_apply(&root, &plan, &opts),
        Err(MigrateError::Interrupted(3))
    ));
    // The context is read from a tree that is now part migrated, yet the same
    // plan completes: the ledgers' pins are those of the ledgers as migrated.
    let report = run_apply(&root, &plan, &accept()).unwrap();
    assert!(report.complete, "{:?}", report.conflicts);
    assert!(report.already_applied <= 3);
    assert_eq!(plan_of(&root).changes, 0);
}

#[test]
fn a_semantic_migration_needs_the_plan_to_be_accepted() {
    let (_dir, root) = seeded();
    let plan = plan_of(&root);
    assert!(matches!(
        run_apply(&root, &plan, &ApplyOptions::default()),
        Err(MigrateError::SemanticNotAccepted(_))
    ));
}

#[test]
fn a_page_it_cannot_migrate_is_listed_and_the_plan_refuses_to_write() {
    let (_dir, root) = seeded();
    // No `generated`: who made the page is not derivable.
    write(
        &root,
        "working/Stampless.md",
        "---\ntype: Note\ntitle: Stampless\npublic: false\nstatus: draft\n---\nBody.\n",
    );
    // Reverse links disagree with the page's own count.
    write(
        &root,
        "working/Topic Y.md",
        "---\ntype: Episode\ntitle: Topic Y\npublic: false\nstatus: stable\ngenerated:\n  by: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:58Z\nsources:\n- id: source\n  resource: free text\ntopic: Topic Y\nepisodes: 27\npromotion-status: candidate_rejected\n---\nBody.\n",
    );
    // A page with no frontmatter at all.
    write(&root, "working/Untitled.md", "just scratch\n");
    let before = tree(&root);
    let plan = plan_of(&root);
    assert_eq!(tree(&root), before);
    assert_eq!(plan.status, PlanStatus::Refused, "{:?}", plan.reasons);
    let refused: BTreeMap<&str, &str> = plan
        .pages
        .iter()
        .filter(|p| p.disposition == Disposition::Refused)
        .map(|p| (p.path.as_str(), p.summary.as_str()))
        .collect();
    assert!(
        refused["working/Stampless.md"].contains("no `generated`"),
        "{refused:?}"
    );
    assert!(
        refused["working/Topic Y.md"].contains("episodes: 27"),
        "{refused:?}"
    );
    assert!(
        refused["working/Untitled.md"].contains("no frontmatter"),
        "{refused:?}"
    );
    assert!(
        plan.reasons
            .iter()
            .any(|r| r.contains("would not validate")),
        "{:?}",
        plan.reasons
    );
    assert!(matches!(
        run_apply(&root, &plan, &accept()),
        Err(MigrateError::PlanRefused(_))
    ));
    assert!(plan.report().contains("not migrated mechanically"));
}

#[test]
fn the_supersession_input_annotates_the_winner_and_keeps_the_loser_as_an_alias() {
    let (_dir, root) = seeded();
    write(
        &root,
        "ontology/m0002-supersessions.yaml",
        "supersessions:\n  - page: Alpha\n    resource: urn:ngm:class:alfa\n    reason: merged\n    at: 2026-09-22T12:41:39Z\n    via: process:apply-decisions/1.0\n    alias: Alfa\n  - page: Missing Winner\n    resource: urn:ngm:class:gone\n    reason: merged\n    at: 2026-09-22T12:41:39Z\n",
    );
    // A winner that is not a page makes the input unusable, and says so.
    let plan = plan_of(&root);
    let input = plan
        .pages
        .iter()
        .find(|p| p.path == "ontology/m0002-supersessions.yaml")
        .expect("the input is listed");
    assert_eq!(input.disposition, Disposition::Refused);
    assert!(
        input.summary.contains("Missing Winner"),
        "{}",
        input.summary
    );

    write(
        &root,
        "ontology/m0002-supersessions.yaml",
        "supersessions:\n  - page: Alpha\n    resource: urn:ngm:class:alfa\n    reason: merged\n    at: 2026-09-22T12:41:39Z\n    via: process:apply-decisions/1.0\n    alias: Alfa\n",
    );
    let plan = plan_of(&root);
    assert_eq!(plan.status, PlanStatus::Ready, "{:?}", plan.reasons);
    run_apply(&root, &plan, &accept()).unwrap();
    let alpha = text(&root, "knowledge/Alpha.md");
    assert!(alpha.contains("supersedes:\n- resource: urn:ngm:class:alfa\n  reason: merged\n  at: 2026-09-22T12:41:39Z\n"), "{alpha}");
    assert!(
        alpha.contains(&format!("  by: {B4}\n  via: process:apply-decisions/1.0\n")),
        "{alpha}"
    );
    assert!(alpha.contains("aliases:\n- Alfa\n"), "{alpha}");
    assert_eq!(
        plan_of(&root).changes,
        0,
        "idempotent with the input in place"
    );
}

#[test]
fn templates_are_listed_for_a_human() {
    let (_dir, root) = seeded();
    write(
        &root,
        "working/templates/Episode.md",
        "---\ntype: Episode\ngenerated: { by: human:jjohare, at: <% tp.date.now(\"x\") %> }\n---\n",
    );
    let plan = plan_of(&root);
    let t = plan
        .pages
        .iter()
        .find(|p| p.path == "working/templates/Episode.md")
        .expect("listed");
    assert_eq!(t.disposition, Disposition::Refused);
    assert!(t.summary.contains("Templater"));
}

fn refused_in(plan: &Plan, path: &str) -> String {
    plan.pages
        .iter()
        .find(|p| p.path == path && p.disposition == Disposition::Refused)
        .unwrap_or_else(|| panic!("{path} is not refused: {:?}", plan.pages))
        .summary
        .clone()
}

#[test]
fn it_never_guesses_a_key_a_kind_or_a_principal() {
    let (_dir, root) = seeded();
    write(
        &root,
        "working/Concept.md",
        "---\ntype: Draft Concept\ntitle: Concept\npublic: false\nstatus: draft\ngenerated:\n  by: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:58Z\n---\nBody.\n",
    );
    // A human key the roster does not list for a human.
    write(
        &root,
        "working/Stranger.md",
        &format!("---\ntype: Note\ntitle: Stranger\npublic: false\nstatus: draft\ngenerated:\n  by: human:{}\n  at: 2026-09-22T12:41:58Z\n---\nBody.\n", "npub126wkhn3fkmf7x7qshycxy9df5ep9hdp65gw3vp6et8hl3wpkvwvs98q46z"),
    );
    // An instrument already named by a stamp that also names an actor in `by`.
    write(
        &root,
        "working/Twice.md",
        "---\ntype: Note\ntitle: Twice\npublic: false\nstatus: draft\ngenerated:\n  by: agent:x/1\n  via: agent:y/2\n  at: 2026-09-22T12:41:58Z\n---\nBody.\n",
    );
    // `public` and `visibility` disagree.
    write(
        &root,
        "working/Split.md",
        "---\ntype: Note\ntitle: Split\npublic: true\nvisibility: shared\nstatus: draft\ngenerated:\n  by: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:58Z\n---\nBody.\n",
    );
    let plan = plan_of(&root);
    assert!(refused_in(&plan, "working/Concept.md").contains("Proposal"));
    assert!(refused_in(&plan, "working/Stranger.md").contains("roster does not list"));
    assert!(refused_in(&plan, "working/Twice.md").contains("already has a `via`"));
    assert!(refused_in(&plan, "working/Split.md").contains("contradicts"));
}

#[test]
fn an_explicit_ledger_list_keeps_its_order_and_is_pinned() {
    let (_dir, root) = seeded();
    write(
        &root,
        "working/Topic Z.md",
        "---\ntype: Note\ntitle: Topic Z\npublic: false\nstatus: draft\ngenerated:\n  by: process:podcast-promote/0.1.0\n  at: 2026-11-21T16:27:43Z\nsource: AI Daily Brief (promotion stage)\nsources:\n- id: slug-b\n  resource: '[[podcast-evidence/slug-b]]'\n- id: slug-a\n  resource: '[[podcast-evidence/slug-a]]'\ntopic: Topic X\nepisodes: 2\npromotion-status: candidate_rejected\n---\nBody.\n",
    );
    let plan = plan_of(&root);
    assert_eq!(
        plan.status,
        PlanStatus::Ready,
        "{:?} {:?}",
        plan.reasons,
        plan.pages
    );
    run_apply(&root, &plan, &accept()).unwrap();
    let z = text(&root, "working/Topic Z.md");
    let (b, a) = (z.find("slug-b").unwrap(), z.find("slug-a").unwrap());
    assert!(b < a, "the author's order is kept: {z}");
    assert_eq!(z.matches("locator: sha256-v1:").count(), 2, "{z}");
    assert!(
        z.contains("type: Brief\n") && !z.contains("\nsource:"),
        "{z}"
    );
    // The pin is the digest of the ledger as it now is: validation finds no stale pin.
    assert_eq!(plan_of(&root).changes, 0);
}

#[test]
fn a_crlf_page_is_migrated_with_its_line_endings() {
    let (_dir, root) = seeded();
    write(
        &root,
        "working/Crlf.md",
        "---\r\ntype: Note\r\ntitle: Crlf\r\npublic: false\r\nstatus: draft\r\ngenerated:\r\n  by: process:vault-migrate/1.0\r\n  at: 2026-09-22T12:41:58Z\r\n---\r\nBody\r\n",
    );
    let plan = plan_of(&root);
    assert_eq!(plan.status, PlanStatus::Ready, "{:?}", plan.reasons);
    run_apply(&root, &plan, &accept()).unwrap();
    let t = fs::read_to_string(root.join("working/Crlf.md")).unwrap();
    assert!(
        t.contains("visibility: shared\r\n") && t.ends_with("---\r\nBody\r\n"),
        "{t:?}"
    );
    assert!(!t.replace("\r\n", "").contains('\n'), "no bare LF: {t:?}");
}

/// The published page API after the migration: Fungible Token is still
/// certified and every page still names who answers for it. (`validate` reading
/// the roster is not enough: the page API reads the record's certification,
/// built through the link-redaction projection.)
#[test]
fn the_published_page_api_keeps_certification_and_provenance_after_the_migration() {
    use ontomark_compile::build::{self, Scope};
    use ontomark_compile::input::vocabulary::Vocabulary;
    use ontomark_compile::whelk::WhelkEl;

    fn build_api(root: &Path, out: &Path) {
        let vocab = Vocabulary::load(root.join("ontology/vocabulary.yaml")).unwrap();
        let opts = build::Options {
            vault_root: root.join("knowledge"),
            out: out.to_path_buf(),
            repo_root: root.to_path_buf(),
            with_markdown_mirror: false,
            generated_at: Some("2026-11-25T10:00:00+00:00".to_owned()),
            source: Some(("0".repeat(40), false)),
            with_working_publish: false,
            redact_private_links: Some(true),
            publish_out: None,
            stale_after: None,
            scope: Scope::Public,
            certificate_out: None,
        };
        build::run(&opts, &profile(), &vocab, &WhelkEl).unwrap();
    }
    fn doc(out: &Path, rel: &str) -> serde_json::Value {
        serde_json::from_slice(&fs::read(out.join(rel)).unwrap()).unwrap()
    }

    let (dir, root) = seeded();
    let before = dir.path().join("before");
    build_api(&root, &before);
    assert_eq!(
        doc(&before, "api/pages/fungible-token.json")["certification"]["state"],
        "certified"
    );

    let plan = plan_of(&root);
    run_apply(&root, &plan, &accept()).unwrap();
    let after = dir.path().join("after");
    build_api(&root, &after);

    let ft = doc(&after, "api/pages/fungible-token.json");
    assert_eq!(ft["certification"]["state"], "certified", "{ft}");
    assert_eq!(ft["provenance"]["wasAttributedTo"], B4, "{ft}");
    let ft3 = doc(&after, "api/v3/pages/fungible-token.json");
    assert_eq!(ft3["certification"]["state"], "certified");

    // The instrument is named in the v3 documents only (their context, v4,
    // has the term); the legacy page API keeps its keys.
    let alpha = doc(&after, "api/pages/alpha.json");
    assert_eq!(alpha["provenance"]["wasAttributedTo"], B4, "{alpha}");
    assert!(alpha["provenance"].get("via").is_none(), "{alpha}");
    let alpha3 = doc(&after, "api/v3/pages/alpha.json");
    assert_eq!(
        alpha3["provenance"]["via"], "agent:enrichment-swarm/6",
        "{alpha3}"
    );
    let ctx = doc(&after, "context/v4.jsonld");
    assert!(ctx["@context"]["provenance"]["@context"]
        .get("via")
        .is_some());
}
