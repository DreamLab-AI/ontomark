//! The file store: atomic replace, exclusive create, confinement, and the
//! two-file atomicity of an approve-then-supersede on a real directory.

mod common;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

use common::*;
use ontomark_compile::input::page::VaultKind;
use ontomark_write::{
    CommitError, FsStore, Move, Resolution, Store, SupersedeReason, TransitionRequest, WriteMode,
};

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn leftovers(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(leftovers(&p));
        } else if p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".ontomark-write-"))
        {
            out.push(p.display().to_string());
        }
    }
    out
}

#[test]
fn replace_is_atomic_and_leaves_no_temporary_file() {
    let d = tmp();
    fs::write(d.path().join("a.md"), "old").unwrap();
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    s.write(
        VaultKind::Working,
        Path::new("a.md"),
        b"new",
        WriteMode::Replace,
    )
    .unwrap();
    assert_eq!(fs::read_to_string(d.path().join("a.md")).unwrap(), "new");
    assert!(leftovers(d.path()).is_empty());
}

#[test]
fn create_new_is_exclusive_and_makes_missing_directories() {
    let d = tmp();
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    s.write(
        VaultKind::Working,
        Path::new("x/y/a.md"),
        b"1",
        WriteMode::CreateNew,
    )
    .unwrap();
    let err = s
        .write(
            VaultKind::Working,
            Path::new("x/y/a.md"),
            b"2",
            WriteMode::CreateNew,
        )
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(d.path().join("x/y/a.md")).unwrap(), "1");
    assert!(
        leftovers(d.path()).is_empty(),
        "a refused create removes its temporary file"
    );
}

#[test]
fn paths_that_leave_the_vault_are_refused() {
    let d = tmp();
    let outside = tmp();
    fs::write(outside.path().join("secret.md"), "s").unwrap();
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    for bad in ["../x.md", "a/../../x.md", "/etc/passwd"] {
        assert!(
            s.write(VaultKind::Working, Path::new(bad), b"x", WriteMode::Replace)
                .is_err(),
            "{bad}"
        );
        assert!(s.read(VaultKind::Working, Path::new(bad)).is_err(), "{bad}");
    }
    // A knowledge root that was not given is not a place to write.
    assert!(s
        .write(
            VaultKind::Knowledge,
            Path::new("a.md"),
            b"x",
            WriteMode::CreateNew
        )
        .is_err());
    assert!(!outside.path().join("x.md").exists());
}

#[test]
fn a_symlinked_directory_that_points_outside_is_never_followed() {
    // `working/assets` is exactly this in the estate.
    let d = tmp();
    let outside = tmp();
    fs::write(outside.path().join("victim.md"), "untouched").unwrap();
    symlink(outside.path(), d.path().join("assets")).unwrap();
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    assert!(s
        .write(
            VaultKind::Working,
            Path::new("assets/victim.md"),
            b"pwned",
            WriteMode::Replace
        )
        .is_err());
    assert!(s
        .write(
            VaultKind::Working,
            Path::new("assets/new.md"),
            b"pwned",
            WriteMode::CreateNew
        )
        .is_err());
    assert!(s
        .read(VaultKind::Working, Path::new("assets/victim.md"))
        .is_err());
    assert!(s
        .remove(VaultKind::Working, Path::new("assets/victim.md"))
        .is_err());
    assert_eq!(
        fs::read_to_string(outside.path().join("victim.md")).unwrap(),
        "untouched"
    );
    assert!(!outside.path().join("new.md").exists());
}

#[test]
fn a_symlinked_file_is_not_followed_either() {
    let d = tmp();
    let outside = tmp();
    fs::write(outside.path().join("t.md"), "untouched").unwrap();
    symlink(outside.path().join("t.md"), d.path().join("link.md")).unwrap();
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    assert!(s
        .write(
            VaultKind::Working,
            Path::new("link.md"),
            b"pwned",
            WriteMode::Replace
        )
        .is_err());
    assert_eq!(
        fs::read_to_string(outside.path().join("t.md")).unwrap(),
        "untouched"
    );
}

#[test]
fn a_vault_root_reached_through_a_symlink_is_fine() {
    let real = tmp();
    let holder = tmp();
    let link = holder.path().join("vault");
    symlink(real.path(), &link).unwrap();
    let mut s = FsStore::new(Some(&link), None).unwrap();
    s.write(
        VaultKind::Working,
        Path::new("a.md"),
        b"ok",
        WriteMode::CreateNew,
    )
    .unwrap();
    assert_eq!(fs::read_to_string(real.path().join("a.md")).unwrap(), "ok");
}

/// Approve-then-supersede on real files, with the second write made to fail by
/// a read-only directory: the first file is put back byte for byte and no
/// temporary file is left anywhere.
#[test]
fn supersede_on_disk_is_all_or_nothing() {
    let d = tmp();
    let a = belief("a1", "approved", "");
    let dr = belief(
        "d1",
        "draft",
        &format!("disputes:\n  - {{ page: urn:visionflow:page:a1, detected: '{T1}' }}\n"),
    );
    fs::create_dir(d.path().join("old")).unwrap();
    fs::write(d.path().join("old/A.md"), &a).unwrap();
    fs::write(d.path().join("D.md"), &dr).unwrap();
    let e = engine(&[("old/A", &a), ("D", &dr)]);
    let plan = e
        .transition(&TransitionRequest {
            id: "D".into(),
            expect: None,
            mv: Move::Approve {
                resolutions: vec![Resolution {
                    page: "urn:visionflow:page:a1".into(),
                    reason: SupersedeReason::WorldChanged,
                    valid_to: None,
                }],
            },
            writer: human(),
            now: T2.into(),
        })
        .unwrap();
    assert_eq!(plan.writes.len(), 2);

    // Make the second write's directory unwritable.
    fs::set_permissions(d.path().join("old"), fs::Permissions::from_mode(0o555)).unwrap();
    let probe = fs::File::create(d.path().join("old/probe"));
    if probe.is_ok() {
        // Running as a user who ignores permissions: the injection cannot work here.
        fs::set_permissions(d.path().join("old"), fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: directory permissions are not enforced for this user");
        return;
    }
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    let err = plan.commit(&mut s).unwrap_err();
    fs::set_permissions(d.path().join("old"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(err, CommitError::Write { ref unrestored, .. } if unrestored.is_empty()),
        "{err}"
    );
    assert_eq!(
        fs::read_to_string(d.path().join("D.md")).unwrap(),
        dr,
        "the first write was undone"
    );
    assert_eq!(fs::read_to_string(d.path().join("old/A.md")).unwrap(), a);
    assert!(leftovers(d.path()).is_empty(), "{:?}", leftovers(d.path()));

    // And once the directory is writable the same plan commits both.
    let mut s = FsStore::new(Some(d.path()), None).unwrap();
    plan.commit(&mut s).unwrap();
    assert!(fs::read_to_string(d.path().join("D.md"))
        .unwrap()
        .contains("status: approved"));
    assert!(fs::read_to_string(d.path().join("old/A.md"))
        .unwrap()
        .contains("status: superseded"));
    assert!(leftovers(d.path()).is_empty());
}
