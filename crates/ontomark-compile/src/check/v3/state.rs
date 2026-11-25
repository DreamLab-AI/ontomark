//! Working-memory shape and review state (ADR-VG-012 §5-§8, ADR-VG-013 §1,
//! §3, §4 and §10; rulings B3, B4, B6 and B9).
//!
//! | code | severity | rule |
//! |---|---|---|
//! | `MISSING_WORKING_KEY` | error | a working page without a key `working_required` (default `page_resource`, `visibility`, `generated`) or its type's `required` asks for |
//! | `INVALID_VISIBILITY` | error | `visibility` outside `private`, `shared`, `public` |
//! | `PRIVATE_UNSUPPORTED` | warning | `visibility: private`, which has no store yet (ADR-VG-012 §5) |
//! | `INVALID_PAGE_RESOURCE` | error | `page_resource` not of the form `urn:visionflow:page:<id>` |
//! | `DUPLICATE_PAGE_RESOURCE` | error | two working pages minted the same `page_resource` |
//! | `MEMORY_STATUS` | error | a memory page without `status`, or outside its set |
//! | `INVALID_TYPE_STATUS` | error | a non-memory type whose vocabulary lists `status` values, and the page uses another |
//! | `SUPERSEDED_BY_MISMATCH` | error | `superseded-by` without `status: superseded`, or the reverse |
//! | `WITHDRAWAL_MISMATCH` | error | a withdrawal entry without `status: invalidated`, or the reverse |
//! | `DISPUTE_NOT_DRAFT` | error | `disputes` on a page that is not `draft` |
//! | `DISPUTE_TARGET` | error | a dispute naming something other than an `approved` `Belief` or `Procedure` of the same visibility, or naming one twice |
//! | `EVIDENCE_NOT_APPROVED` | warning | `rel: evidence` to a memory page that is not `approved` |
//! | `INVALID_ABOUT` | error | an `about` value that is neither a wikilink nor an IRI |
//! | `UNRESOLVED_ABOUT` | error | an `about` wikilink or page urn naming no page in either vault (only when both are loaded) |
//!
//! The transition table (`ILLEGAL_TRANSITION`) is the write engine's, not
//! `validate`'s: it needs the previous version of the page.
//!
//! Pages with no frontmatter at all are private scratch (they are never
//! public) and are skipped, as are journals.

use std::collections::{HashMap, HashSet};

use serde_norway::Value as Yaml;

use crate::check::validate::{Report, Severity};
use crate::input::frontmatter::Wikilink;
use crate::input::page::{Page, Vault, VaultKind};
use crate::input::time::parse_utc_seconds;
use crate::input::vocabulary::Vocabulary;

use super::temporal::{entries, is_iri, is_memory, superseded_by, PAGE_URN};
use super::Context;

/// The memory lifecycle of ADR-VG-013 §1.
pub const MEMORY_STATUSES: [&str; 5] =
    ["draft", "approved", "superseded", "invalidated", "archived"];

/// The three `visibility` values (ADR-VG-012 §5).
pub const VISIBILITIES: [&str; 3] = ["private", "shared", "public"];

/// The keys every working page carries when the vocabulary does not say.
pub const DEFAULT_REQUIRED: [&str; 3] = ["page_resource", "visibility", "generated"];

/// `true` when `page` is published/indexed as public under `vocab`: a working
/// page by `visibility: public` under version 3 (ruling B9), every other page
/// by `public: true`. Under version 2 `public: true` is honoured everywhere.
#[must_use]
pub fn is_public_under(page: &Page, kind: VaultKind, vocab: &Vocabulary) -> bool {
    if vocab.is_v3() && kind == VaultKind::Working {
        page.frontmatter.text("visibility").as_deref() == Some("public")
    } else {
        page.is_public()
    }
}

/// [`is_public_under`] where the vocabulary may be unknown: with `None` a page
/// is public by `public: true`, the version 2 rule.
#[must_use]
pub fn is_public_in(page: &Page, kind: VaultKind, vocab: Option<&Vocabulary>) -> bool {
    vocab.map_or_else(|| page.is_public(), |v| is_public_under(page, kind, v))
}

/// `true` when current recall may return `page` at `now` (Unix seconds):
/// an `approved` memory kind (`Episode`, `Belief`, `Procedure`) whose
/// `valid-from` is not after `now` (absent reads as started, ruling B5) and
/// whose `valid-to` is not at or before `now` (ADR-VG-010 §6, ADR-VG-013 §1).
/// Always `false` under version 2, which has no recall rule.
#[must_use]
pub fn is_recallable(page: &Page, vocab: &Vocabulary, now: i64) -> bool {
    if !vocab.is_v3() {
        return false;
    }
    let Some(ty) = page.frontmatter.text("type") else {
        return false;
    };
    if !matches!(ty.as_str(), "Episode" | "Belief" | "Procedure") || !vocab.working_type(&ty).memory
    {
        return false;
    }
    if page.frontmatter.text("status").as_deref() != Some("approved") {
        return false;
    }
    let at = |key: &str| {
        page.frontmatter
            .text(key)
            .and_then(|s| parse_utc_seconds(&s))
    };
    at("valid-from").is_none_or(|t| t <= now) && at("valid-to").is_none_or(|t| t > now)
}

fn err(report: &mut Report, path: &str, code: &str, message: impl Into<String>) {
    report.push(path, Severity::Error, code, message);
}

fn has_frontmatter(page: &Page) -> bool {
    page.frontmatter.keys().next().is_some()
}

/// Per-page rules for working pages.
pub(crate) fn check_page(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    if ctx.vault.kind != VaultKind::Working || !has_frontmatter(page) {
        return;
    }
    let path = page.id.as_str();
    let fm = &page.frontmatter;
    let ty = fm.text("type").unwrap_or_default();
    let def = vocab.working_type(&ty);

    // Required keys: working-wide, then the type's own.
    let wide: Vec<String> = vocab
        .working_required
        .clone()
        .unwrap_or_else(|| DEFAULT_REQUIRED.iter().map(|s| (*s).to_owned()).collect());
    let mut seen = HashSet::new();
    for key in wide
        .iter()
        .chain(&def.required)
        .filter(|k| seen.insert(k.as_str()))
    {
        if !fm.has(key) {
            err(
                report,
                path,
                "MISSING_WORKING_KEY",
                format!("a working page needs `{key}`"),
            );
        }
    }

    if let Some(v) = fm.get("visibility") {
        match v.as_str() {
            Some("private") => report.push(
                path,
                Severity::Warning,
                "PRIVATE_UNSUPPORTED",
                "`visibility: private` has no store yet; no page may be marked private (ADR-VG-012 §5)",
            ),
            Some(s) if VISIBILITIES.contains(&s) => {}
            _ => err(
                report,
                path,
                "INVALID_VISIBILITY",
                format!("`visibility` must be one of {}", VISIBILITIES.join(", ")),
            ),
        }
    }
    if let Some(v) = fm.get("page_resource") {
        let ok = v
            .as_str()
            .and_then(|s| s.strip_prefix(PAGE_URN))
            .is_some_and(|id| !id.is_empty() && !id.contains(char::is_whitespace));
        if !ok {
            err(
                report,
                path,
                "INVALID_PAGE_RESOURCE",
                format!("`page_resource` must be `{PAGE_URN}<id>`"),
            );
        }
    }

    status_rules(report, page, &ty, vocab, ctx);
    review_state(report, page, vocab, ctx);
    about(report, page, ctx);
}

fn status_rules(report: &mut Report, page: &Page, ty: &str, vocab: &Vocabulary, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let status = page.frontmatter.text("status");
    let def = vocab.working_type(ty);
    if is_memory(page, vocab, ctx) {
        let set: Vec<&str> = if def.status.is_empty() {
            MEMORY_STATUSES.to_vec()
        } else {
            def.status.iter().map(String::as_str).collect()
        };
        match status.as_deref() {
            None => err(
                report,
                path,
                "MEMORY_STATUS",
                format!("`type: {ty}` requires a `status`"),
            ),
            Some(s) if !set.contains(&s) => err(
                report,
                path,
                "MEMORY_STATUS",
                format!("`status: {s}` is not one of {}", set.join(", ")),
            ),
            Some(_) => {}
        }
    } else if !def.status.is_empty() {
        if let Some(s) = status
            .as_deref()
            .filter(|s| !def.status.iter().any(|d| d == s))
        {
            err(
                report,
                path,
                "INVALID_TYPE_STATUS",
                format!(
                    "`status: {s}` is not one of {} for `type: {ty}`",
                    def.status.join(", ")
                ),
            );
        }
    }
}

fn review_state(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    let path = page.id.as_str();
    if !is_memory(page, vocab, ctx) {
        return;
    }
    let status = page.frontmatter.text("status").unwrap_or_default();
    let has_successor = !superseded_by(page).is_empty();
    if has_successor != (status == "superseded") && (has_successor || status == "superseded") {
        err(
            report,
            path,
            "SUPERSEDED_BY_MISMATCH",
            "`superseded-by` and `status: superseded` go together",
        );
    }
    let withdrawn = entries(page).iter().any(|e| e.resource.is_none());
    if withdrawn != (status == "invalidated") {
        err(
            report,
            path,
            "WITHDRAWAL_MISMATCH",
            "a withdrawal entry (a `supersedes` entry without `resource`) and `status: invalidated` go together",
        );
    }
    if let Some(d) = page.frontmatter.get("disputes") {
        if status != "draft" {
            err(
                report,
                path,
                "DISPUTE_NOT_DRAFT",
                "`disputes` belong on a `draft` page",
            );
        }
        let visibility = page
            .frontmatter
            .text("visibility")
            .unwrap_or_else(|| "shared".into());
        let mut seen = HashSet::new();
        for reference in d
            .as_sequence()
            .into_iter()
            .flatten()
            .filter_map(Yaml::as_str)
        {
            let target = ctx.resolve(reference);
            let ok = target.is_some_and(|t| {
                let ty = t.page.frontmatter.text("type").unwrap_or_default();
                matches!(ty.as_str(), "Belief" | "Procedure")
                    && t.page.frontmatter.text("status").as_deref() == Some("approved")
                    && t.page
                        .frontmatter
                        .text("visibility")
                        .unwrap_or_else(|| "shared".into())
                        == visibility
            });
            let first = target.map_or_else(|| reference.to_owned(), |t| t.page.id.clone());
            if !ok || !seen.insert(first) {
                err(
                    report,
                    path,
                    "DISPUTE_TARGET",
                    format!("`{reference}` is not a distinct approved Belief or Procedure of the same visibility"),
                );
            }
        }
    }
    for source in page
        .okf()
        .sources
        .iter()
        .filter(|s| s.rel.as_deref() == Some("evidence"))
    {
        let Some(t) = ctx.resolve(&source.resource) else {
            continue;
        };
        let ty = t.page.frontmatter.text("type").unwrap_or_default();
        if t.kind == VaultKind::Working
            && vocab.working_type(&ty).memory
            && t.page.frontmatter.text("status").as_deref() != Some("approved")
        {
            report.push(
                path,
                Severity::Warning,
                "EVIDENCE_NOT_APPROVED",
                format!("evidence `{}` is not approved", t.page.id),
            );
        }
    }
}

/// B3: `about` resolves to a knowledge class or a working page, and fails only
/// when dangling.
fn about(report: &mut Report, page: &Page, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let Some(value) = page.frontmatter.get("about") else {
        return;
    };
    let refs: Vec<&str> = match value {
        Yaml::Sequence(items) => items.iter().filter_map(Yaml::as_str).collect(),
        other => other.as_str().into_iter().collect(),
    };
    for reference in refs {
        let link = Wikilink::parse(reference).is_some();
        if !link && !is_iri(reference) {
            err(
                report,
                path,
                "INVALID_ABOUT",
                format!("`about: {reference}` must be a wikilink or an IRI"),
            );
        } else if (link || reference.starts_with(PAGE_URN))
            && ctx.sees(VaultKind::Knowledge)
            && ctx.sees(VaultKind::Working)
            && ctx.resolve(reference).is_none()
        {
            err(
                report,
                path,
                "UNRESOLVED_ABOUT",
                format!("`about: {reference}` names no page"),
            );
        }
    }
}

/// Vault-wide: `page_resource` minted twice.
pub(crate) fn check_corpus(report: &mut Report, vault: &Vault) {
    if vault.kind != VaultKind::Working {
        return;
    }
    let mut owners: HashMap<String, Vec<&str>> = HashMap::new();
    for p in vault.pages.iter().filter(|p| has_frontmatter(p)) {
        if let Some(r) = p.frontmatter.text("page_resource") {
            owners.entry(r).or_default().push(&p.id);
        }
    }
    let mut dups: Vec<_> = owners.into_iter().filter(|(_, v)| v.len() > 1).collect();
    dups.sort();
    for (resource, ids) in dups {
        for id in &ids {
            err(
                report,
                id,
                "DUPLICATE_PAGE_RESOURCE",
                format!("`{resource}` is also minted by {}", ids.join(", ")),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::check::validate::Report;
    use crate::input::page::VaultKind;

    const B4: &str = "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";

    fn head(ty: &str, status: &str, extra: &str) -> String {
        format!("---\ntype: {ty}\nstatus: {status}\npage_resource: urn:visionflow:page:0a1b\nvisibility: shared\ngenerated: {{ by: {B4}, at: '2026-11-01T00:00:00Z' }}\n{extra}---\nbody\n")
    }

    fn w(pages: &[(&str, &str)]) -> Report {
        run(pages, VaultKind::Working, &vocab(3, ""))
    }

    fn ev() -> &'static str {
        "about: '[[X]]'\nconfidence: 0.5\nsources:\n  - { id: a, resource: 'https://x.example', rel: evidence, locator: p1 }\n"
    }

    #[test]
    fn a_well_formed_working_page_is_clean_of_state_codes() {
        let r = w(&[("N", &head("Note", "draft", ""))]);
        for c in [
            "MISSING_WORKING_KEY",
            "INVALID_VISIBILITY",
            "INVALID_PAGE_RESOURCE",
            "MEMORY_STATUS",
        ] {
            assert!(!has(&r, c), "{c}: {:?}", codes(&r));
        }
    }

    #[test]
    fn the_three_working_keys_are_required_and_frontmatterless_pages_are_skipped() {
        let r = w(&[("N", "---\ntype: Note\nstatus: draft\n---\nb\n")]);
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| i.code == "MISSING_WORKING_KEY")
                .count(),
            3
        );
        assert!(!has(&w(&[("S", "just scratch\n")]), "MISSING_WORKING_KEY"));
        let custom = Vocabulary_with("working_required: [page_resource]\n");
        let r = run(
            &[(
                "N",
                "---\ntype: Note\nstatus: draft\npage_resource: urn:visionflow:page:1\n---\n",
            )],
            VaultKind::Working,
            &custom,
        );
        assert!(!has(&r, "MISSING_WORKING_KEY"));
    }

    #[allow(non_snake_case)]
    fn Vocabulary_with(extra: &str) -> crate::input::vocabulary::Vocabulary {
        vocab(3, extra)
    }

    #[test]
    fn visibility_is_closed_and_private_is_not_yet_usable() {
        let page = |v: &str| {
            head("Note", "draft", "").replace("visibility: shared", &format!("visibility: {v}"))
        };
        assert!(!has(&w(&[("N", &page("public"))]), "INVALID_VISIBILITY"));
        assert!(has(&w(&[("N", &page("everyone"))]), "INVALID_VISIBILITY"));
        assert!(has(&w(&[("N", &page("private"))]), "PRIVATE_UNSUPPORTED"));
    }

    #[test]
    fn page_resource_has_the_urn_form_and_is_unique() {
        let bad =
            head("Note", "draft", "").replace("urn:visionflow:page:0a1b", "'https://x.example/p'");
        assert!(has(&w(&[("N", &bad)]), "INVALID_PAGE_RESOURCE"));
        let empty =
            head("Note", "draft", "").replace("urn:visionflow:page:0a1b", "'urn:visionflow:page:'");
        assert!(has(&w(&[("N", &empty)]), "INVALID_PAGE_RESOURCE"));
        let r = w(&[
            ("A", &head("Note", "draft", "")),
            ("B", &head("Note", "draft", "")),
        ]);
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| i.code == "DUPLICATE_PAGE_RESOURCE")
                .count(),
            2
        );
    }

    #[test]
    fn memory_kinds_use_the_lifecycle_and_others_keep_the_old_set() {
        for s in MEMORY_STATUSES {
            let r = w(&[("B", &head("Belief", s, ev()))]);
            assert!(
                !has(&r, "MEMORY_STATUS") && !has(&r, "INVALID_STATUS"),
                "{s}: {:?}",
                codes(&r)
            );
        }
        assert!(has(
            &w(&[("B", &head("Belief", "stable", ev()))]),
            "MEMORY_STATUS"
        ));
        assert!(has(
            &w(&[("B", &head("Belief", "deprecated", ev()))]),
            "MEMORY_STATUS"
        ));
        assert!(has(
            &w(&[("B", "---\ntype: Belief\n---\n")]),
            "MEMORY_STATUS"
        ));
        // A Note keeps draft | stable | deprecated, and `approved` is invalid for it.
        assert!(!has(
            &w(&[("N", &head("Note", "stable", ""))]),
            "INVALID_STATUS"
        ));
        assert!(has(
            &w(&[("N", &head("Note", "approved", ""))]),
            "INVALID_STATUS"
        ));
        // Under v2 the memory set is not known: `approved` stays invalid.
        let v2 = run(
            &[("B", &head("Belief", "approved", ""))],
            VaultKind::Working,
            &vocab(2, ""),
        );
        assert!(has(&v2, "INVALID_STATUS"));
    }

    #[test]
    fn a_type_may_state_its_own_status_set() {
        let v = vocab(3, "").clone();
        let v = {
            let mut v = v;
            v.working_type_defs.insert(
                "Note".into(),
                crate::input::vocabulary::WorkingTypeDef {
                    status: vec!["draft".into(), "final".into()],
                    ..Default::default()
                },
            );
            v
        };
        assert!(has(
            &run(
                &[("N", &head("Note", "stable", ""))],
                VaultKind::Working,
                &v
            ),
            "INVALID_TYPE_STATUS"
        ));
        assert!(!has(
            &run(&[("N", &head("Note", "final", ""))], VaultKind::Working, &v),
            "INVALID_TYPE_STATUS"
        ));
    }

    #[test]
    fn superseded_by_and_withdrawal_entries_pair_with_their_status() {
        let sb = "superseded-by: ['urn:visionflow:page:9']\n";
        assert!(!has(
            &w(&[("B", &head("Belief", "superseded", &format!("{sb}{}", ev())))]),
            "SUPERSEDED_BY_MISMATCH"
        ));
        assert!(has(
            &w(&[("B", &head("Belief", "approved", &format!("{sb}{}", ev())))]),
            "SUPERSEDED_BY_MISMATCH"
        ));
        assert!(has(
            &w(&[("B", &head("Belief", "superseded", ev()))]),
            "SUPERSEDED_BY_MISMATCH"
        ));
        let wd = format!(
            "supersedes:\n  - {{ reason: withdrawn, at: '2026-11-21T00:00:00Z', by: {B4} }}\n"
        );
        assert!(!has(
            &w(&[(
                "B",
                &head("Belief", "invalidated", &format!("{wd}{}", ev()))
            )]),
            "WITHDRAWAL_MISMATCH"
        ));
        assert!(has(
            &w(&[("B", &head("Belief", "approved", &format!("{wd}{}", ev())))]),
            "WITHDRAWAL_MISMATCH"
        ));
        assert!(has(
            &w(&[("B", &head("Belief", "invalidated", ev()))]),
            "WITHDRAWAL_MISMATCH"
        ));
    }

    #[test]
    fn disputes_name_distinct_approved_beliefs_of_the_same_visibility() {
        let approved = head("Belief", "approved", ev()).replace("0a1b", "aa");
        let ok = w(&[
            ("Old", &approved),
            (
                "New",
                &head(
                    "Belief",
                    "draft",
                    &format!("disputes: ['[[Old]]']\n{}", ev()),
                )
                .replace("0a1b", "bb"),
            ),
        ]);
        assert!(
            !has(&ok, "DISPUTE_TARGET") && !has(&ok, "DISPUTE_NOT_DRAFT"),
            "{:?}",
            codes(&ok)
        );
        let dup = w(&[
            ("Old", &approved),
            (
                "New",
                &head(
                    "Belief",
                    "draft",
                    &format!("disputes: ['[[Old]]', '[[Old]]']\n{}", ev()),
                )
                .replace("0a1b", "bb"),
            ),
        ]);
        assert!(has(&dup, "DISPUTE_TARGET"));
        let not_draft = w(&[
            ("Old", &approved),
            (
                "New",
                &head(
                    "Belief",
                    "approved",
                    &format!("disputes: ['[[Old]]']\n{}", ev()),
                )
                .replace("0a1b", "bb"),
            ),
        ]);
        assert!(has(&not_draft, "DISPUTE_NOT_DRAFT"));
        let note = w(&[
            ("Old", &head("Note", "draft", "")),
            (
                "New",
                &head(
                    "Belief",
                    "draft",
                    &format!("disputes: ['[[Old]]']\n{}", ev()),
                )
                .replace("0a1b", "bb"),
            ),
        ]);
        assert!(has(&note, "DISPUTE_TARGET"));
        let public = approved.replace("visibility: shared", "visibility: public");
        let vis = w(&[
            ("Old", &public),
            (
                "New",
                &head(
                    "Belief",
                    "draft",
                    &format!("disputes: ['[[Old]]']\n{}", ev()),
                )
                .replace("0a1b", "bb"),
            ),
        ]);
        assert!(has(&vis, "DISPUTE_TARGET"));
    }

    #[test]
    fn evidence_in_an_unapproved_memory_page_warns() {
        let ledger = head("Episode", "draft", "");
        let src = "about: '[[X]]'\nconfidence: 0.5\nsources:\n  - { id: a, resource: '[[L]]', rel: evidence, locator: 'p1' }\n";
        let r = w(&[
            ("L", &ledger),
            ("B", &head("Belief", "draft", src).replace("0a1b", "bb")),
        ]);
        assert!(has(&r, "EVIDENCE_NOT_APPROVED"), "{:?}", codes(&r));
        let approved = w(&[
            ("L", &head("Episode", "approved", "")),
            ("B", &head("Belief", "draft", src).replace("0a1b", "bb")),
        ]);
        assert!(!has(&approved, "EVIDENCE_NOT_APPROVED"));
    }

    #[test]
    fn about_must_be_a_link_or_iri_and_resolve_when_both_vaults_are_loaded() {
        let page = head("Belief", "draft", "about: '[[Gone]]'\nconfidence: 0.5\nsources:\n  - { id: a, resource: 'https://x.example', rel: evidence, locator: p1 }\n");
        assert!(
            !has(&w(&[("B", &page)]), "UNRESOLVED_ABOUT"),
            "one vault: skip"
        );
        let know = vault_of(&[("K", "---\ntype: Class\n---\n")], VaultKind::Knowledge);
        let work = vault_of(&[("B", &page)], VaultKind::Working);
        let sib = super::super::Siblings {
            vaults: vec![&know],
            roster: None,
        };
        assert!(has(
            &run_with(&work, &vocab(3, ""), &sib),
            "UNRESOLVED_ABOUT"
        ));
        let ok = page.replace("[[Gone]]", "[[K]]");
        let work = vault_of(&[("B", &ok)], VaultKind::Working);
        assert!(!has(
            &run_with(&work, &vocab(3, ""), &sib),
            "UNRESOLVED_ABOUT"
        ));
        let work = vault_of(
            &[("B", &page.replace("'[[Gone]]'", "plain text"))],
            VaultKind::Working,
        );
        assert!(has(&run_with(&work, &vocab(3, ""), &sib), "INVALID_ABOUT"));
    }

    #[test]
    fn public_true_counts_as_public_under_v2_and_visibility_under_v3() {
        let text = "---\ntype: Note\npublic: true\nvisibility: shared\n---\n";
        let page = crate::input::page::Page::parse("/v/N.md", "N.md", "N", text).unwrap();
        assert!(is_public_under(&page, VaultKind::Working, &vocab(2, "")));
        assert!(!is_public_under(&page, VaultKind::Working, &vocab(3, "")));
        let shared = crate::input::page::Page::parse(
            "/v/N.md",
            "N.md",
            "N",
            "---\nvisibility: public\n---\n",
        )
        .unwrap();
        assert!(is_public_under(&shared, VaultKind::Working, &vocab(3, "")));
        assert!(!is_public_under(&shared, VaultKind::Working, &vocab(2, "")));
        // Knowledge always reads `public: true`.
        assert!(is_public_under(&page, VaultKind::Knowledge, &vocab(3, "")));
        let r = run(&[("N", text)], VaultKind::Working, &vocab(3, ""));
        assert_eq!(r.public_pages, 0);
        let r = run(&[("N", text)], VaultKind::Working, &vocab(2, ""));
        assert_eq!(r.public_pages, 1);
    }

    /// The record form of the recall flag (`V3Facts::recallable_at`, what the
    /// page API publishes) agrees with the page form on every case.
    #[test]
    fn the_record_recall_flag_agrees_with_the_page_flag() {
        let v3 = vocab(3, "");
        let now = 1_795_255_200;
        for (ty, status, extra) in [
            ("Belief", "approved", ""),
            ("Episode", "approved", "valid-from: 2026-01-01T00:00:00Z\n"),
            ("Belief", "approved", "valid-from: 2030-01-01T00:00:00Z\n"),
            ("Belief", "approved", "valid-to: 2026-11-21T10:00:00Z\n"),
            ("Belief", "approved", "valid-to: 2030-01-01T00:00:00Z\n"),
            ("Belief", "draft", ""),
            ("Brief", "approved", ""),
            ("Note", "approved", ""),
            ("Belief", "superseded", ""),
        ] {
            let text = format!("---\ntype: {ty}\nstatus: {status}\n{extra}---\n");
            let page = crate::input::page::Page::parse("/v/P.md", "P.md", "P", &text).unwrap();
            let vault = crate::input::page::Vault {
                root: std::path::PathBuf::from("/v"),
                kind: VaultKind::Working,
                pages: vec![page.clone()],
                journals: Vec::new(),
                skipped: Vec::new(),
            };
            let corpus = crate::ir::model::Corpus::build(&vault, &v3);
            let facts = corpus.records[0].v3.as_deref().expect("v3 facts");
            assert_eq!(
                facts.recallable_at(now),
                is_recallable(&page, &v3, now),
                "{ty} {status} {extra}"
            );
        }
    }

    #[test]
    fn recall_returns_current_approved_memory_only() {
        let p = |ty: &str, status: &str, extra: &str| {
            crate::input::page::Page::parse(
                "/v/P.md",
                "P.md",
                "P",
                &format!("---\ntype: {ty}\nstatus: {status}\n{extra}---\n"),
            )
            .unwrap()
        };
        let v3 = vocab(3, "");
        let now = 1_795_255_200;
        assert!(is_recallable(&p("Belief", "approved", ""), &v3, now));
        assert!(is_recallable(
            &p("Episode", "approved", "valid-from: 2026-01-01T00:00:00Z\n"),
            &v3,
            now
        ));
        assert!(!is_recallable(
            &p("Belief", "approved", "valid-from: 2030-01-01T00:00:00Z\n"),
            &v3,
            now
        ));
        assert!(
            !is_recallable(
                &p("Belief", "approved", "valid-to: 2026-11-21T10:00:00Z\n"),
                &v3,
                now
            ),
            "closed at now"
        );
        assert!(is_recallable(
            &p("Belief", "approved", "valid-to: 2030-01-01T00:00:00Z\n"),
            &v3,
            now
        ));
        for (ty, status) in [
            ("Belief", "draft"),
            ("Brief", "approved"),
            ("Proposal", "approved"),
            ("Note", "approved"),
            ("Belief", "superseded"),
        ] {
            assert!(
                !is_recallable(&p(ty, status, ""), &v3, now),
                "{ty} {status}"
            );
        }
        assert!(!is_recallable(
            &p("Belief", "approved", ""),
            &vocab(2, ""),
            now
        ));
    }
}
