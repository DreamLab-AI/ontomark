//! Reproductions for the SEV-3 register items S3.2 (a symlinked page is dropped
//! without a `Skip` record) and S3.3 (an unclosed opening `---` is read as "no
//! frontmatter"). Each test fails today for the register's stated reason.

use ontomark_compile::input::page::walk_pages;
use ontomark_compile::ir::projection::{inspect_inputs, PublicationBlocked};

/// S3.2: `walk_pages` must report a symlinked `.md` instead of dropping it
/// (page.rs:501-503 `continue`s on any non-file with no `Skip`).
#[cfg(unix)]
#[test]
fn s3_2_symlinked_page_is_reported_not_dropped() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let pages = scratch.path().join("pages");
    std::fs::create_dir_all(&pages).expect("pages dir");
    std::fs::write(pages.join("a.md"), "---\npublic: true\n---\n# A\n").expect("write a.md");
    std::os::unix::fs::symlink(pages.join("a.md"), pages.join("b.md")).expect("symlink b.md");

    let walk = walk_pages(&pages).expect("walk");

    assert!(
        walk.skipped.iter().any(|s| s.path.as_path() == std::path::Path::new("b.md")),
        "S3.2: symlinked b.md was neither loaded nor reported; files={:?} skipped={:?}",
        walk.files,
        walk.skipped
    );

    // The invariant stated at page.rs:79-81: every markdown file on disk is
    // either loaded or reported as skipped.
    let mut on_disk: Vec<String> = std::fs::read_dir(&pages)
        .expect("list pages")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    on_disk.sort();
    let mut accounted: Vec<String> = walk
        .files
        .iter()
        .map(|p| p.strip_prefix(&pages).unwrap_or(p).to_string_lossy().into_owned())
        .chain(walk.skipped.iter().map(|s| s.path.to_string_lossy().into_owned()))
        .collect();
    accounted.sort();
    assert_eq!(
        accounted, on_disk,
        "S3.2: every markdown file must be loaded or reported as skipped"
    );
}

/// S3.3: a page whose opening `---` is never closed is malformed input, not a
/// frontmatter-less page. Today `split` returns `yaml: None`, so the census
/// counts it `excluded_no_frontmatter` and returns `Ok`.
#[test]
fn s3_3_unclosed_frontmatter_is_invalid_input_not_private() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let pages = scratch.path().join("pages");
    std::fs::create_dir_all(&pages).expect("pages dir");
    std::fs::write(
        pages.join("broken.md"),
        "---\ntitle: X\npublic: true\n\n# Body, but the fence never closed\n",
    )
    .expect("write broken.md");

    match inspect_inputs(&pages) {
        Err(PublicationBlocked::Census(counts)) => assert!(
            counts.contains("invalid_input"),
            "S3.3: refused, but not as invalid_input: {counts}"
        ),
        Err(other) => panic!("S3.3: wrong refusal: {other}"),
        Ok(census) => panic!(
            "S3.3: unclosed frontmatter accepted silently (public={}, private={}, excluded={}, counts={:?})",
            census.public_pages, census.non_public_pages, census.excluded, census.counts
        ),
    }
}
