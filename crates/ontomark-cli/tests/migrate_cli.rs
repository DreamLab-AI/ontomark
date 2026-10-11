//! The `ontomark migrate` command layer. `migrate_cmd.rs` is compiled here
//! directly so it is linted and tested before `main.rs` wires the subcommand.

#[path = "../src/migrate_cmd.rs"]
mod migrate_cmd;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use migrate_cmd::MigrateCommand;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: MigrateCommand,
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

fn repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/estate-small"),
        &root,
    );
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles/estate/profile.yaml"),
        root.join("profile.yaml"),
    )
    .unwrap();
    let page = root.join("knowledge/pages/5G.md");
    let mut text = fs::read_to_string(&page).unwrap();
    text.push_str("\n- ## Overview\n\t- A point\n");
    fs::write(page, text).unwrap();
    (dir, root)
}

fn run(root: &Path, args: &[&str]) -> ExitCode {
    let mut argv = vec!["migrate"];
    argv.extend_from_slice(args);
    let cli = Cli::try_parse_from(argv).unwrap();
    migrate_cmd::run(&cli.command, root, true).unwrap()
}

#[test]
fn plan_apply_status_rollback_through_the_command_layer() {
    let (_d, root) = repo();
    let before = fs::read(root.join("knowledge/pages/5G.md")).unwrap();
    let plan = root.join("plan.json");
    let plan_arg = plan.to_str().unwrap();

    assert_eq!(run(&root, &["list"]), ExitCode::SUCCESS);
    assert_eq!(
        run(&root, &["plan", "m0001-obsidian-bodies", "--out", plan_arg]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        fs::read(root.join("knowledge/pages/5G.md")).unwrap(),
        before
    );

    assert_eq!(
        run(&root, &["apply", "--plan", plan_arg]),
        ExitCode::SUCCESS
    );
    assert_ne!(
        fs::read(root.join("knowledge/pages/5G.md")).unwrap(),
        before
    );
    assert_eq!(run(&root, &["status"]), ExitCode::SUCCESS);

    // Second plan is empty.
    let again = run(&root, &["plan", "m0001-obsidian-bodies", "--out", plan_arg]);
    assert_eq!(again, ExitCode::SUCCESS);
    assert!(fs::read_to_string(&plan)
        .unwrap()
        .contains("\"changes\": 0"));

    let digest = fs::read_dir(root.join(".ontomark/migrations/m0001-obsidian-bodies"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name()
        .into_string()
        .unwrap();
    assert_eq!(
        run(&root, &["rollback", "m0001-obsidian-bodies", &digest]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        fs::read(root.join("knowledge/pages/5G.md")).unwrap(),
        before
    );
}

#[test]
fn a_stale_plan_exits_one_and_leaves_the_edited_page() {
    let (_d, root) = repo();
    let plan = root.join("plan.json");
    let plan_arg = plan.to_str().unwrap();
    run(&root, &["plan", "m0001-obsidian-bodies", "--out", plan_arg]);
    let page = root.join("knowledge/pages/5G.md");
    let mut text = fs::read_to_string(&page).unwrap();
    text.push_str("- late edit\n");
    fs::write(&page, &text).unwrap();
    assert_eq!(
        run(&root, &["apply", "--plan", plan_arg]),
        ExitCode::from(1)
    );
    assert_eq!(fs::read_to_string(&page).unwrap(), text);
}

/// Needs the `Migrate` variant in `main.rs`.
#[test]
#[ignore = "enabled when main.rs wires migrate"]
fn the_binary_runs_migrate_end_to_end() {
    let (_d, root) = repo();
    let exe = env!("CARGO_BIN_EXE_ontomark");
    let plan = root.join("plan.json");
    let run = |args: &[&str]| {
        std::process::Command::new(exe)
            .arg("--repo")
            .arg(&root)
            .arg("migrate")
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(&[
        "plan",
        "m0001-obsidian-bodies",
        "--out",
        plan.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&["apply", "--plan", plan.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&["plan", "m0001-obsidian-bodies"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("0 to change"));
}
