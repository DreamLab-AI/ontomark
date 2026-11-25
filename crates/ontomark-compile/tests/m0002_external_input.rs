//! `m0002-vocabulary-v3` taking its supersessions input from outside the tree
//! (`ONTOMARK_M0002_SUPERSESSIONS`), so nothing about the refit is committed
//! into the data. This is its own test binary because the variable is process
//! global: no other test may see it.

use std::fs;
use std::path::Path;

use ontomark_compile::input::page::VaultKind;
use ontomark_compile::migrate::{self, apply_with, plan, ApplyOptions, PlanStatus};
use ontomark_compile::profile::Profile;

const BOTH: [VaultKind; 2] = [VaultKind::Knowledge, VaultKind::Working];
const VAR: &str = "ONTOMARK_M0002_SUPERSESSIONS";

fn profile() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml")).unwrap()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn seeded(root: &Path) {
    write(root, "working/Scratch.md", "just scratch\n");
    write(
        root,
        "ontology/vocabulary.yaml",
        include_str!("m0002/vocabulary-v2.yaml"),
    );
    write(
        root,
        "ontology/attribution.yaml",
        include_str!("m0002/attribution-v2.yaml"),
    );
    write(
        root,
        "knowledge/Alpha.md",
        "---\ntype: Class\ntitle: Alpha\nresource: urn:ngm:class:alpha\npublic: true\nstatus: draft\ngenerated:\n  by: agent:enrichment-swarm/6\n  process: process:vault-migrate/1.0\n  at: 2026-09-22T12:41:39Z\n---\nBody.\n",
    );
}

const INPUT: &str = "supersessions:\n  - page: Alpha\n    resource: urn:ngm:class:alfa\n    reason: merged\n    at: 2026-09-22T12:41:39Z\n    alias: Alfa\n";

// One test, run in order: the variable is process global.
#[test]
fn the_input_may_live_outside_the_tree_and_its_digest_binds_the_plan_and_inside_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    seeded(&root);
    let input = dir.path().join("supersessions.yaml");
    fs::write(&input, INPUT).unwrap();
    let m = migrate::lookup("m0002-vocabulary-v3").unwrap();

    std::env::set_var(VAR, &input);
    let with = plan(&root, &BOTH, m.as_ref(), &profile()).unwrap();
    assert_eq!(with.status, PlanStatus::Ready, "{:?}", with.reasons);
    assert!(
        !with
            .pages
            .iter()
            .any(|p| p.path == "ontology/m0002-supersessions.yaml"),
        "nothing about the input is listed in the tree"
    );
    // Applying writes the supersession into the winner and leaves no file
    // for the input in the tree.
    apply_with(
        &root,
        &with,
        m.as_ref(),
        &ApplyOptions {
            accept_semantic: true,
            ..ApplyOptions::default()
        },
    )
    .unwrap();
    let alpha = fs::read_to_string(root.join("knowledge/Alpha.md")).unwrap();
    assert!(alpha.contains("resource: urn:ngm:class:alfa"), "{alpha}");
    assert!(alpha.contains("- Alfa"), "{alpha}");
    assert!(!root.join("ontology/m0002-supersessions.yaml").exists());

    // The digest of the input is part of the plan: another input, another digest.
    let other = dir.path().join("other.yaml");
    fs::write(&other, INPUT.replace("Alfa", "Alpha-old")).unwrap();
    let repo2 = dir.path().join("repo2");
    seeded(&repo2);
    std::env::set_var(VAR, &input);
    let d1 = plan(&repo2, &BOTH, m.as_ref(), &profile())
        .unwrap()
        .input_digest;
    std::env::set_var(VAR, &other);
    let d2 = plan(&repo2, &BOTH, m.as_ref(), &profile())
        .unwrap()
        .input_digest;
    std::env::remove_var(VAR);
    let d0 = plan(&repo2, &BOTH, m.as_ref(), &profile())
        .unwrap()
        .input_digest;
    assert_ne!(d1, d2);
    assert_ne!(d1, d0);

    // An input inside the tree, or one that is missing, is refused.
    let root = dir.path().join("repo3");
    seeded(&root);
    write(&root, "notes/sup.yaml", INPUT);
    std::env::set_var(VAR, root.join("notes/sup.yaml"));
    let inside = plan(&root, &BOTH, m.as_ref(), &profile());
    std::env::set_var(VAR, dir.path().join("absent.yaml"));
    let absent = plan(&root, &BOTH, m.as_ref(), &profile());
    std::env::remove_var(VAR);
    let e = inside.expect_err("inside the tree");
    assert!(e.to_string().contains("outside"), "{e}");
    let e = absent.expect_err("missing");
    assert!(e.to_string().contains(VAR), "{e}");
}
