//! EXP-08: migrations are reversible and idempotent; a second run has no
//! changes; an interrupted run recovers; source edits invalidate a stale
//! patch. Every test works on a temporary copy of `conformance/fixtures/estate-small`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ontomark_compile::input::page::VaultKind;
use ontomark_compile::migrate::{
    self, apply, apply_with, m0001::ObsidianBodies, plan, rollback, status, ApplyOptions,
    Disposition, Kind, MigrateError, Migration, Plan, PlanStatus, Transform,
};
use ontomark_compile::profile::Profile;

const BOTH: [VaultKind; 2] = [VaultKind::Knowledge, VaultKind::Working];

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml")).unwrap()
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            fs::copy(e.path(), to).unwrap();
        }
    }
}

const OUTLINE: &str = "\n- ## Overview\n\t- Point one \u{2014} caf\u{e9} \u{65e5}\u{672c}\u{8a9e}\n\t- ![img.png](assets/img.png){:height 10, :width 20}\n\t- TODO write it\n";

/// A temporary repository: the estate-small fixture plus synthetic
/// Logseq-bodied pages in both vaults.
fn seeded() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/estate-small"),
        &root,
    );
    for name in ["5G", "3D Model", "ACL"] {
        let p = root.join(format!("knowledge/{name}.md"));
        let mut text = fs::read_to_string(&p).unwrap();
        text.push_str(OUTLINE);
        fs::write(p, text).unwrap();
    }
    // A working page with a YAML comment, an HTML comment outside the converted
    // region, and Unicode.
    fs::write(
        root.join("working/Comment Note.md"),
        "---\ntype: Note\ntitle: Comment Note\n# keep this comment \u{1f600}\npublic: false\n---\n<!-- keep this too -->\nA definition paragraph with caf\u{e9}.\n\n- ## Heading\n\t- Child\n",
    )
    .unwrap();
    // A CRLF page.
    fs::write(
        root.join("working/Crlf Note.md"),
        "---\r\ntype: Note\r\ntitle: Crlf Note\r\npublic: false\r\n---\r\n- ## Heading\r\n\t- Child \u{e9}\r\n",
    )
    .unwrap();
    // A page that mixes line endings.
    fs::write(
        root.join("working/Mixed Note.md"),
        "---\ntype: Note\ntitle: Mixed Note\npublic: false\n---\r\n- ## Heading\n\t- Child\r\n",
    )
    .unwrap();
    // A journal page.
    fs::write(
        root.join("working/journals/2026-10-12.md"),
        "---\ntype: Journal\ntitle: 2026-10-12\npublic: false\n---\n- Did a thing\n\t- Detail\n",
    )
    .unwrap();
    (dir, root)
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                if e.file_name() != ".ontomark" {
                    walk(base, &p, out);
                }
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn plan_m1(root: &Path) -> Plan {
    plan(root, &BOTH, &ObsidianBodies, &profile()).unwrap()
}

fn changed_paths(p: &Plan) -> Vec<&str> {
    p.pages
        .iter()
        .filter(|x| x.disposition == Disposition::Change)
        .map(|x| x.path.as_str())
        .collect()
}

#[test]
fn plan_finds_the_logseq_pages_and_is_ready() {
    let (_d, root) = seeded();
    let p = plan_m1(&root);
    assert_eq!(p.status, PlanStatus::Ready, "{:?}", p.reasons);
    let changed = changed_paths(&p);
    for want in [
        "knowledge/5G.md",
        "knowledge/3D Model.md",
        "knowledge/ACL.md",
        "working/Comment Note.md",
        "working/Crlf Note.md",
        "working/journals/2026-10-12.md",
    ] {
        assert!(changed.contains(&want), "{want} missing from {changed:?}");
    }
    assert_eq!(p.refused, 1, "only the mixed-ending page is refused");
    assert!(p.deltas.compiled, "{:?}", p.deltas.notes);
    assert!(p.deltas.visibility.is_empty() && p.deltas.identity.is_empty());
}

#[test]
fn plan_is_deterministic_and_holds_no_page_text() {
    let (_d, root) = seeded();
    let a = plan_m1(&root).to_json().unwrap();
    let b = plan_m1(&root).to_json().unwrap();
    assert_eq!(a, b);
    assert!(!a.contains("caf\u{e9}") && !a.contains("Point one"));
    assert_eq!(Plan::from_json(&a).unwrap().to_json().unwrap(), a);
}

#[test]
fn plan_agrees_with_repair_bodies_page_for_page() {
    let (_d, root) = seeded();
    let p = plan_m1(&root);
    let mut from_plan: Vec<String> = changed_paths(&p)
        .into_iter()
        .filter(|x| !x.contains("Crlf"))
        .map(str::to_owned)
        .collect();
    from_plan.sort();
    let scopes = ontomark_compile::check::repair::scopes_for(
        &[
            (VaultKind::Knowledge, &root.join("knowledge")),
            (VaultKind::Working, &root.join("working")),
        ],
        ontomark_compile::input::page::EXCLUDED_DIRS,
    );
    let report = ontomark_compile::check::bodies::run(&ontomark_compile::check::bodies::Options {
        scopes,
        dry_run: true,
    })
    .unwrap();
    let mut from_repair: Vec<String> = report
        .changes
        .iter()
        .map(|c| {
            c.path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        // `repair bodies` rewrites the mixed-ending page; the migration refuses it.
        .filter(|x| !x.contains("Crlf") && !x.contains("Mixed"))
        .collect();
    from_repair.sort();
    assert_eq!(from_plan, from_repair);
}

#[test]
fn apply_then_replan_is_idempotent() {
    let (_d, root) = seeded();
    let p = plan_m1(&root);
    let r = apply(&root, &p).unwrap();
    assert!(r.complete, "{:?}", r.conflicts);
    assert_eq!(r.applied, p.changes);
    let again = plan_m1(&root);
    assert_eq!(again.changes, 0, "second plan: {:?}", changed_paths(&again));
    assert_eq!(again.status, PlanStatus::Ready);
    // Re-applying the old plan is a no-op: every page is already at `after`.
    let r2 = apply(&root, &p).unwrap();
    assert_eq!((r2.applied, r2.already_applied), (0, p.changes));
}

#[test]
fn interrupted_apply_resumes_to_the_same_final_bytes() {
    let (_d1, reference) = seeded();
    apply(&reference, &plan_m1(&reference)).unwrap();

    for die_before_commit_record in [false, true] {
        let (_d2, root) = seeded();
        let p = plan_m1(&root);
        let opts = ApplyOptions {
            stop_after: Some(2),
            die_before_commit_record,
            ..ApplyOptions::default()
        };
        let m = ObsidianBodies;
        let err = apply_with(&root, &p, &m, &opts).unwrap_err();
        assert!(matches!(err, MigrateError::Interrupted(_)), "{err}");
        assert_eq!(status(&root).unwrap()[0].state, "interrupted");
        assert_ne!(tree(&root), tree(&reference), "run was really cut short");

        let r = apply(&root, &p).unwrap();
        assert!(r.complete, "{:?}", r.conflicts);
        assert!(r.already_applied >= 2, "resumed past finished pages");
        assert_eq!(
            tree(&root),
            tree(&reference),
            "die_before_commit={die_before_commit_record}"
        );
        assert_eq!(status(&root).unwrap()[0].state, "complete");
    }
}

#[test]
fn a_torn_journal_tail_does_not_stop_a_restart() {
    let (_d, root) = seeded();
    let p = plan_m1(&root);
    let opts = ApplyOptions {
        stop_after: Some(1),
        ..ApplyOptions::default()
    };
    apply_with(&root, &p, &ObsidianBodies, &opts).unwrap_err();
    let journal = root
        .join(".ontomark/migrations/m0001-obsidian-bodies")
        .join(&p.input_digest)
        .join("journal.jsonl");
    let mut text = fs::read_to_string(&journal).unwrap();
    text.push_str("{\"event\":\"intent\",\"path\":\"knowl");
    fs::write(&journal, text).unwrap();
    let r = apply(&root, &p).unwrap();
    assert!(r.complete, "{:?}", r.conflicts);
    assert_eq!(plan_m1(&root).changes, 0);
}

#[test]
fn a_page_edited_after_planning_conflicts_and_the_rest_apply() {
    let (_d, root) = seeded();
    let p = plan_m1(&root);
    let edited = root.join("knowledge/5G.md");
    let mut text = fs::read_to_string(&edited).unwrap();
    text.push_str("- an edit made after the plan\n");
    fs::write(&edited, &text).unwrap();

    let r = apply(&root, &p).unwrap();
    assert!(!r.complete);
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].path, "knowledge/5G.md");
    assert_eq!(
        fs::read_to_string(&edited).unwrap(),
        text,
        "stale page untouched"
    );
    assert_eq!(r.applied, p.changes - 1);
    // Re-planning picks the edited page up afresh.
    let fresh = plan_m1(&root);
    assert_eq!(changed_paths(&fresh), ["knowledge/5G.md"]);
}

#[test]
fn rollback_restores_byte_identical_originals() {
    let (_d, root) = seeded();
    let pristine = tree(&root);
    let p = plan_m1(&root);
    apply(&root, &p).unwrap();
    assert_ne!(tree(&root), pristine);

    let j = &status(&root).unwrap()[0];
    let r = rollback(&root, &j.dir).unwrap();
    assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
    assert_eq!(r.restored, p.changes);
    assert_eq!(tree(&root), pristine);
    assert_eq!(status(&root).unwrap()[0].state, "rolled-back");
    // Rolling back twice is harmless.
    let r2 = rollback(&root, &j.dir).unwrap();
    assert_eq!((r2.restored, r2.already_original), (0, p.changes));
    // And the migration can be planned and applied again from the restored state.
    assert_eq!(plan_m1(&root).changes, p.changes);
}

#[test]
fn rollback_after_a_crash_between_rename_and_commit_restores_the_page_in_the_window() {
    let (_d, root) = seeded();
    let pristine = tree(&root);
    let p = plan_m1(&root);
    let opts = ApplyOptions {
        stop_after: Some(2),
        die_before_commit_record: true,
        ..ApplyOptions::default()
    };
    apply_with(&root, &p, &ObsidianBodies, &opts).unwrap_err();
    // Three pages differ from the originals: two committed, one in the window.
    let changed = tree(&root)
        .iter()
        .filter(|(k, v)| pristine.get(*k) != Some(*v))
        .count();
    assert_eq!(changed, 3);

    let j = &status(&root).unwrap()[0];
    assert_eq!((j.intents, j.applied), (3, 2));
    let r = rollback(&root, &j.dir).unwrap();
    assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
    assert_eq!(r.restored, 3);
    assert_eq!(tree(&root), pristine);
}

#[test]
fn a_stale_run_is_reported_as_complete_with_conflicts() {
    let (_d, root) = seeded();
    let p = plan_m1(&root);
    fs::write(
        root.join("knowledge/5G.md"),
        "---\ntype: Note\n---\nedited\n",
    )
    .unwrap();
    apply(&root, &p).unwrap();
    let s = &status(&root).unwrap()[0];
    assert_eq!(s.state, "complete-with-conflicts");
    assert_eq!((s.conflicts, s.applied), (1, p.changes - 1));
}

#[test]
fn a_replan_with_nothing_to_do_reports_zero_inferred_not_unknown() {
    let (_d, root) = seeded();
    apply(&root, &plan_m1(&root)).unwrap();
    let again = plan_m1(&root);
    #[cfg(feature = "whelk-el")]
    {
        let i = again.deltas.inferred.expect("zero, not absent");
        assert_eq!((i.added, i.removed), (0, 0));
    }
    #[cfg(not(feature = "whelk-el"))]
    assert!(
        again.deltas.inferred.is_none(),
        "no classifier, no inference claim"
    );
}

#[test]
fn a_syntax_migration_that_changes_the_page_iri_is_refused() {
    let (_d, root) = seeded();
    fn change_page_iri(s: &str) -> Option<String> {
        s.contains("page_resource: urn:visionflow:page:5-g\n")
            .then(|| {
                s.replace(
                    "page_resource: urn:visionflow:page:5-g\n",
                    "page_resource: urn:visionflow:page:5-g-x\n",
                )
            })
    }
    let m = Edit {
        kind: Kind::Syntax,
        f: change_page_iri,
    };
    let p = plan(&root, &BOTH, &m, &profile()).unwrap();
    assert_eq!(p.status, PlanStatus::Refused);
    assert!(p.deltas.identity.iter().any(|c| c.field == "page_resource"));
}

#[test]
fn rollback_refuses_a_page_edited_after_apply() {
    let (_d, root) = seeded();
    let pristine = tree(&root);
    let p = plan_m1(&root);
    apply(&root, &p).unwrap();
    let edited = root.join("knowledge/ACL.md");
    let mut text = fs::read_to_string(&edited).unwrap();
    text.push_str("\nA later human edit.\n");
    fs::write(&edited, &text).unwrap();

    let j = &status(&root).unwrap()[0];
    let r = rollback(&root, &j.dir).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].path, "knowledge/ACL.md");
    assert_eq!(r.restored, p.changes - 1);
    assert_eq!(fs::read_to_string(&edited).unwrap(), text);
    let mut now = tree(&root);
    let mut want = pristine;
    now.remove("knowledge/ACL.md");
    want.remove("knowledge/ACL.md");
    assert_eq!(now, want);
    assert_eq!(status(&root).unwrap()[0].state, "rollback-partial");
}

#[test]
fn unicode_crlf_and_comments_outside_the_region_survive_byte_for_byte() {
    let (_d, root) = seeded();
    let before = tree(&root);
    apply(&root, &plan_m1(&root)).unwrap();
    let after = tree(&root);

    let c = String::from_utf8(after["working/Comment Note.md"].clone()).unwrap();
    assert!(c.starts_with(
        "---\ntype: Note\ntitle: Comment Note\n# keep this comment \u{1f600}\npublic: false\n---\n<!-- keep this too -->\nA definition paragraph with caf\u{e9}.\n"
    ), "{c:?}");
    assert!(c.contains("## Heading") && !c.contains("- ## Heading"));

    let k = String::from_utf8(after["knowledge/5G.md"].clone()).unwrap();
    assert!(k.contains("Point one \u{2014} caf\u{e9} \u{65e5}\u{672c}\u{8a9e}"));
    assert!(k.contains("![img.png|20](assets/img.png)"));
    // Frontmatter bytes are identical.
    let head = |b: &[u8]| {
        let s = String::from_utf8(b.to_vec()).unwrap();
        s[..s.find("\n---\n").unwrap()].to_owned()
    };
    assert_eq!(
        head(&before["knowledge/5G.md"]),
        head(&after["knowledge/5G.md"])
    );

    let crlf = &after["working/Crlf Note.md"];
    let text = String::from_utf8(crlf.clone()).unwrap();
    assert!(text.starts_with("---\r\ntype: Note\r\ntitle: Crlf Note\r\npublic: false\r\n---\r\n"));
    assert!(text.contains("Child \u{e9}"));
    assert_eq!(
        text.matches('\n').count(),
        text.matches("\r\n").count(),
        "no bare LF"
    );

    // The mixed-ending page is refused and so left alone.
    assert_eq!(
        before["working/Mixed Note.md"],
        after["working/Mixed Note.md"]
    );
    // Pages with no outline are untouched.
    assert_eq!(
        before["knowledge/3D Asset.md"],
        after["knowledge/3D Asset.md"]
    );
}

/// A migration over knowledge pages for plan-machinery tests.
struct Edit {
    kind: Kind,
    f: fn(&str) -> Option<String>,
}

impl Migration for Edit {
    fn id(&self) -> &str {
        "test-edit"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &str {
        "test migration"
    }
    fn kind(&self) -> Kind {
        self.kind
    }
    fn transform(&self, rel: &str, source: &str) -> Transform {
        if !rel.starts_with("knowledge/") {
            return Transform::Unchanged;
        }
        match (self.f)(source) {
            Some(source) => Transform::Changed {
                source,
                summary: "test edit".to_owned(),
            },
            None => Transform::Unchanged,
        }
    }
}

fn append_comment(s: &str) -> Option<String> {
    (!s.ends_with("<!-- t -->\n")).then(|| format!("{s}\n<!-- t -->\n"))
}

fn rename_resource(s: &str) -> Option<String> {
    s.contains("resource: urn:ngm:class:5-g\n").then(|| {
        s.replace(
            "resource: urn:ngm:class:5-g\n",
            "resource: urn:ngm:class:5-g-renamed\n",
        )
    })
}

fn go_private(s: &str) -> Option<String> {
    s.contains("public: true\n")
        .then(|| s.replacen("public: true\n", "public: false\n", 1))
}

#[test]
fn the_delta_machinery_reports_zero_for_a_no_op_edit() {
    let (_d, root) = seeded();
    let m = Edit {
        kind: Kind::Syntax,
        f: append_comment,
    };
    let p = plan(&root, &BOTH, &m, &profile()).unwrap();
    assert!(p.changes > 10);
    assert_eq!(p.status, PlanStatus::Ready, "{:?}", p.reasons);
    let d = &p.deltas;
    assert!(d.compiled, "{:?}", d.notes);
    assert_eq!(
        (d.asserted.added, d.asserted.removed),
        (0, 0),
        "{:?}",
        d.asserted
    );
    #[cfg(feature = "whelk-el")]
    {
        let inferred = d.inferred.as_ref().expect("whelk is a default feature");
        assert_eq!((inferred.added, inferred.removed), (0, 0));
    }
    #[cfg(not(feature = "whelk-el"))]
    assert!(d.inferred.is_none(), "no classifier, no inference claim");
    assert!(d.visibility.is_empty() && d.identity.is_empty() && d.newly_unsatisfiable.is_empty());
}

#[test]
fn a_syntax_migration_that_changes_a_resource_iri_is_refused() {
    let (_d, root) = seeded();
    let before = tree(&root);
    let m = Edit {
        kind: Kind::Syntax,
        f: rename_resource,
    };
    let p = plan(&root, &BOTH, &m, &profile()).unwrap();
    assert_eq!(p.status, PlanStatus::Refused);
    assert!(
        p.reasons.iter().any(|r| r.contains("identity")),
        "{:?}",
        p.reasons
    );
    assert!(p.deltas.identity.iter().any(|c| c.field == "resource"));
    assert_eq!(p.deltas.resources_removed, ["urn:ngm:class:5-g"]);
    let err = apply_with(&root, &p, &m, &ApplyOptions::default()).unwrap_err();
    assert!(matches!(err, MigrateError::PlanRefused(_)), "{err}");
    assert_eq!(tree(&root), before, "a refused plan writes nothing");
}

#[test]
fn a_syntax_migration_that_changes_visibility_is_refused() {
    let (_d, root) = seeded();
    let m = Edit {
        kind: Kind::Syntax,
        f: go_private,
    };
    let p = plan(&root, &BOTH, &m, &profile()).unwrap();
    assert_eq!(p.status, PlanStatus::Refused);
    assert!(!p.deltas.visibility.is_empty());
    assert!(p.deltas.visibility.iter().all(|v| v.before && !v.after));
    assert!(
        p.deltas.asserted.removed > 0,
        "private pages leave the public graph"
    );
}

#[test]
fn a_semantic_migration_is_planned_with_deltas_and_needs_explicit_acceptance() {
    let (_d, root) = seeded();
    let m = Edit {
        kind: Kind::Semantic,
        f: rename_resource,
    };
    let p = plan(&root, &BOTH, &m, &profile()).unwrap();
    assert_eq!(p.status, PlanStatus::Ready);
    assert!(!p.deltas.identity.is_empty());
    let err = apply_with(&root, &p, &m, &ApplyOptions::default()).unwrap_err();
    assert!(matches!(err, MigrateError::SemanticNotAccepted(_)), "{err}");
    let ok = ApplyOptions {
        accept_semantic: true,
        ..ApplyOptions::default()
    };
    assert!(apply_with(&root, &p, &m, &ok).unwrap().complete);
}

#[test]
fn a_plan_from_another_migration_or_version_is_not_applied() {
    let (_d, root) = seeded();
    let mut p = plan_m1(&root);
    p.migration.version += 1;
    let err = apply(&root, &p).unwrap_err();
    assert!(matches!(err, MigrateError::PlanMismatch { .. }), "{err}");
    p.migration.version -= 1;
    p.migration.id = "m9999-nonexistent".to_owned();
    assert!(matches!(
        apply(&root, &p).unwrap_err(),
        MigrateError::UnknownMigration(_)
    ));
}

#[test]
fn a_hostile_plan_path_cannot_escape_the_repository() {
    let (_d, root) = seeded();
    let mut p = plan_m1(&root);
    p.pages[0].path = "../outside.md".to_owned();
    let r = apply(&root, &p).unwrap();
    assert!(r.conflicts.iter().any(|c| c.path == "../outside.md"));
}

#[test]
fn the_registry_lists_m0001() {
    let ids: Vec<String> = migrate::registry()
        .iter()
        .map(|m| m.id().to_owned())
        .collect();
    assert_eq!(ids, ["m0001-obsidian-bodies"]);
    assert!(migrate::lookup("m0001-obsidian-bodies").is_some());
}
