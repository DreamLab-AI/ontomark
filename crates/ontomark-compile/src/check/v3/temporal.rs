//! Authored time, supersession and statements (ADR-VG-010 §2-§4, ADR-VG-013
//! §1 and §4, rulings B1 and B3).
//!
//! | code | severity | rule |
//! |---|---|---|
//! | `INVALID_DATETIME` | error | a time key or entry time is not `YYYY-MM-DDTHH:MM:SSZ` |
//! | `VALID_RANGE` | warning | `valid-to` is not after `valid-from` (the interval is half-open) |
//! | `VALID_TO_STATUS` | error | `valid-to` without the status that closes it: `deprecated`, or `superseded` / `archived` on a memory kind |
//! | `INVALID_SUPERSEDES` | error | a `supersedes` or `superseded-by` value outside the grammar |
//! | `DANGLING_SUPERSESSION` | warning | a wikilink or page urn that names no loaded page |
//! | `SUPERSESSION_CLOCK` | warning | a `world-changed` entry whose target carries no `valid-to` |
//! | `DEPRECATED_NO_SUCCESSOR` | error | `deprecated` with neither `superseded-by` nor its own withdrawal entry (`corrected` or `withdrawn`, no `resource`) |
//! | `SUPERSESSION_CYCLE` | error | pages that supersede one another in a ring |
//! | `SUPERSEDED_TARGET` | error | a live page's relation points at a deprecated page |
//! | `INVALID_STATEMENT` | error | a `statements` entry outside the grammar |
//! | `STATEMENT_MISMATCH` | error | an open statement absent from its live list, or a closed one still in it |
//! | `UNKNOWN_SOURCE_ID` | warning | a statement cites a source id the page does not declare |
//!
//! Which clock a reason closes is [`clocks_closed`], and only that function.

use std::collections::{HashMap, HashSet};

use serde_norway::{Mapping, Value as Yaml};

use crate::check::validate::{Report, Severity};
use crate::input::frontmatter::Wikilink;
use crate::input::page::{Page, Vault, VaultKind};
use crate::input::time::{clocks_closed, parse_utc_seconds, REASONS};
use crate::input::vocabulary::Vocabulary;

use super::stamps::{check_entry, Shape};
use super::Context;

/// The prefix of a working page's `page_resource` (ADR-VG-012 §8).
pub const PAGE_URN: &str = "urn:visionflow:page:";

/// A `supersedes` entry that names a reason, read tolerantly.
#[derive(Debug)]
struct Entry {
    resource: Option<String>,
    reason: Option<String>,
}

/// The `supersedes` entries of a page that are mappings.
fn entries(page: &Page) -> Vec<Entry> {
    let Some(Yaml::Sequence(items)) = page.frontmatter.get("supersedes") else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Yaml::as_mapping)
        .map(|m| Entry {
            resource: text(m, "resource"),
            reason: text(m, "reason"),
        })
        .collect()
}

fn text(m: &Mapping, key: &str) -> Option<String> {
    m.get(Yaml::String(key.into()))
        .and_then(Yaml::as_str)
        .map(str::to_owned)
}

/// The `superseded-by` references of a page.
fn superseded_by(page: &Page) -> Vec<String> {
    page.frontmatter.strings("superseded-by")
}

pub(crate) fn is_iri(s: &str) -> bool {
    let s = s.trim();
    s.split_once(':').is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// Per-page temporal rules.
pub(crate) fn check_page(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let fm = &page.frontmatter;

    // --- valid time -----------------------------------------------------
    let mut instant = |key: &str| -> Option<i64> {
        let value = fm.get(key)?;
        let parsed = value.as_str().and_then(parse_utc_seconds);
        if parsed.is_none() {
            report.push(
                path,
                Severity::Error,
                "INVALID_DATETIME",
                format!("`{key}` must be an xsd:dateTime in UTC at second precision, like 2026-11-21T00:00:00Z"),
            );
        }
        parsed
    };
    let from = instant("valid-from");
    let to = instant("valid-to");
    if let (Some(from), Some(to)) = (from, to) {
        if to <= from {
            report.push(
                path,
                Severity::Warning,
                "VALID_RANGE",
                "`valid-to` is not after `valid-from`; the interval is half-open and this one is empty",
            );
        }
    }
    if fm.has("valid-to") {
        valid_to_status(report, page, vocab, ctx);
    }

    // --- supersession ---------------------------------------------------
    supersedes(report, page, ctx);
    superseded_by_refs(report, page, ctx);
    let status = fm.text("status");
    let memory = is_memory(page, vocab, ctx);
    if !memory && status.as_deref() == Some("deprecated") {
        let own_withdrawal = entries(page).iter().any(|e| {
            e.resource.is_none() && matches!(e.reason.as_deref(), Some("withdrawn" | "corrected"))
        });
        if superseded_by(page).is_empty() && !own_withdrawal {
            report.push(
                path,
                Severity::Error,
                "DEPRECATED_NO_SUCCESSOR",
                "`status: deprecated` needs `superseded-by`, or a `supersedes` entry with \
                 `reason: withdrawn` (or `corrected`) and no `resource` on the page itself",
            );
        }
    }

    statements(report, page, vocab, ctx);
}

/// `true` when `page` is a durable memory kind in the working vault.
pub(crate) fn is_memory(page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) -> bool {
    ctx.vault.kind == VaultKind::Working
        && page
            .frontmatter
            .text("type")
            .is_some_and(|t| vocab.working_type(&t).memory)
}

/// ADR-VG-010 §2 and ADR-VG-013 §1: `valid-to` needs the status that closes it.
fn valid_to_status(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    let status = page.frontmatter.text("status");
    let (allowed, what): (&[&str], &str) = if is_memory(page, vocab, ctx) {
        (&["superseded", "archived"], "`superseded` or `archived`")
    } else {
        (&["deprecated"], "`deprecated`")
    };
    if !status.as_deref().is_some_and(|s| allowed.contains(&s)) {
        report.push(
            &page.id,
            Severity::Error,
            "VALID_TO_STATUS",
            format!(
                "`valid-to` requires `status: {what}`, not {}",
                status.as_deref().unwrap_or("(absent)")
            ),
        );
    }
}

/// The grammar of a reference to another page, by the vault it is written in
/// (ruling B3): a working page names its target by `page_resource` urn; a
/// knowledge page by wikilink, or by IRI when the target was folded away.
fn reference_form_ok(reference: &str, kind: VaultKind) -> bool {
    match kind {
        VaultKind::Working => reference.trim().starts_with(PAGE_URN),
        VaultKind::Knowledge => Wikilink::parse(reference).is_some() || is_iri(reference),
    }
}

/// Warn when a reference that must name a live page does not. An IRI into
/// `knowledge/` may legitimately name a page that was folded away, so only
/// wikilinks and page urns are judged.
fn check_resolves(report: &mut Report, path: &str, what: &str, reference: &str, ctx: &Context<'_>) {
    let judged = if Wikilink::parse(reference).is_some() {
        Some(ctx.vault.kind)
    } else if reference.trim().starts_with(PAGE_URN) {
        Some(VaultKind::Working)
    } else {
        None
    };
    if judged.is_some_and(|kind| ctx.sees(kind)) && ctx.resolve(reference).is_none() {
        report.push(
            path,
            Severity::Warning,
            "DANGLING_SUPERSESSION",
            format!("{what} `{reference}` names no page"),
        );
    }
}

fn invalid(report: &mut Report, path: &str, field: &str, message: impl std::fmt::Display) {
    report.push(
        path,
        Severity::Error,
        "INVALID_SUPERSEDES",
        format!("`{field}`: {message}"),
    );
}

fn supersedes(report: &mut Report, page: &Page, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let Some(value) = page.frontmatter.get("supersedes") else {
        return;
    };
    let Yaml::Sequence(items) = value else {
        invalid(report, path, "supersedes", "must be a list of entries");
        return;
    };
    for (i, item) in items.iter().enumerate() {
        let field = format!("supersedes[{i}]");
        let Some(m) = item.as_mapping() else {
            invalid(report, path, &field, "must be a mapping");
            continue;
        };
        let reason = text(m, "reason");
        match reason.as_deref() {
            None => invalid(report, path, &field, "`reason` is required"),
            Some(r) if !REASONS.contains(&r) => invalid(
                report,
                path,
                &field,
                format!("`reason: {r}` is not one of {}", REASONS.join(", ")),
            ),
            Some(_) => {}
        }
        match text(m, "resource") {
            None => {
                if !matches!(reason.as_deref(), Some("corrected" | "withdrawn")) {
                    invalid(
                        report,
                        path,
                        &field,
                        "an entry without `resource` is the page's own withdrawal, \
                         so `reason` must be `corrected` or `withdrawn`",
                    );
                }
            }
            Some(resource) => {
                if !reference_form_ok(&resource, ctx.vault.kind) {
                    let want = match ctx.vault.kind {
                        VaultKind::Working => "a page_resource urn (urn:visionflow:page:<id>)",
                        VaultKind::Knowledge => "a wikilink or an IRI",
                    };
                    invalid(
                        report,
                        path,
                        &field,
                        format!("`resource: {resource}` must be {want} in this vault"),
                    );
                } else {
                    check_resolves(report, path, &format!("`{field}.resource`"), &resource, ctx);
                    clock_check(report, path, &field, reason.as_deref(), &resource, ctx);
                }
            }
        }
        if let Some(at) = m.get(Yaml::String("at".into())) {
            if at.as_str().and_then(parse_utc_seconds).is_none() {
                report.push(
                    path,
                    Severity::Error,
                    "INVALID_DATETIME",
                    format!("`{field}.at` must be an xsd:dateTime in UTC at second precision"),
                );
            }
        }
        // Who and when follow the stamp grammar; both are required here.
        super::stamps::check_entry(
            report,
            path,
            &field,
            m,
            Shape {
                require_by: true,
                require_at: true,
                verified: false,
            },
            ctx.roster(),
        );
    }
}

/// A `world-changed` entry closes valid time, so the page it replaces should
/// say when (`valid-to`). The check reads the one clock function.
fn clock_check(
    report: &mut Report,
    path: &str,
    field: &str,
    reason: Option<&str>,
    resource: &str,
    ctx: &Context<'_>,
) {
    if !reason.and_then(clocks_closed).is_some_and(|c| c.valid) {
        return;
    }
    if let Some(target) = ctx.resolve(resource) {
        if !target.page.frontmatter.has("valid-to") {
            report.push(
                path,
                Severity::Warning,
                "SUPERSESSION_CLOCK",
                format!(
                    "`{field}` gives `world-changed`, which closes valid time, but `{}` \
                     carries no `valid-to`",
                    target.page.id
                ),
            );
        }
    }
}

fn superseded_by_refs(report: &mut Report, page: &Page, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let Some(value) = page.frontmatter.get("superseded-by") else {
        return;
    };
    let refs = match value {
        Yaml::Sequence(items) => items.iter().map(Yaml::as_str).collect::<Vec<_>>(),
        other => vec![other.as_str()],
    };
    for reference in refs {
        let Some(reference) = reference else {
            invalid(report, path, "superseded-by", "entries must be text");
            continue;
        };
        if !reference_form_ok(reference, ctx.vault.kind) {
            invalid(
                report,
                path,
                "superseded-by",
                format!("`{reference}` is not a valid reference in this vault"),
            );
        } else {
            check_resolves(report, path, "`superseded-by`", reference, ctx);
        }
    }
}

/// The normal form two statement values are compared in: a wikilink's
/// lower-cased target, else the trimmed text.
fn normal(value: &str) -> String {
    Wikilink::parse(value).map_or_else(|| value.trim().to_owned(), |l| l.target.to_lowercase())
}

fn statements(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let Some(value) = page.frontmatter.get("statements") else {
        return;
    };
    let bad = |report: &mut Report, field: &str, message: &str| {
        report.push(
            path,
            Severity::Error,
            "INVALID_STATEMENT",
            format!("`{field}`: {message}"),
        );
    };
    let Yaml::Sequence(items) = value else {
        bad(report, "statements", "must be a list of entries");
        return;
    };
    let source_ids: HashSet<String> = page.okf().sources.iter().map(|s| s.id.clone()).collect();
    for (i, item) in items.iter().enumerate() {
        let field = format!("statements[{i}]");
        let Some(m) = item.as_mapping() else {
            bad(report, &field, "must be a mapping");
            continue;
        };
        let (Some(key), Some(value)) = (text(m, "key"), text(m, "value")) else {
            bad(report, &field, "`key` and `value` are required text");
            continue;
        };
        if !vocab.relations.contains_key(&key) {
            bad(
                report,
                &field,
                &format!("`key: {key}` is not a relation in the vocabulary"),
            );
            continue;
        }
        let mut when = |name: &str| -> Option<Option<i64>> {
            m.get(Yaml::String(name.into())).map(|v| {
                let parsed = v.as_str().and_then(parse_utc_seconds);
                if parsed.is_none() {
                    report.push(
                        path,
                        Severity::Error,
                        "INVALID_DATETIME",
                        format!(
                            "`{field}.{name}` must be an xsd:dateTime in UTC at second precision"
                        ),
                    );
                }
                parsed
            })
        };
        let from = when("valid-from");
        let to = when("valid-to");
        if let (Some(Some(from)), Some(Some(to))) = (from, to) {
            if to <= from {
                report.push(
                    path,
                    Severity::Warning,
                    "VALID_RANGE",
                    format!("`{field}`: `valid-to` is not after `valid-from`"),
                );
            }
        }
        if let Some(reason) = text(m, "reason") {
            if !REASONS.contains(&reason.as_str()) {
                bad(
                    report,
                    &field,
                    &format!("`reason: {reason}` is not one of {}", REASONS.join(", ")),
                );
            }
        }
        // A closed entry records an act, so it carries who and when.
        let closed = to.is_some();
        if closed {
            check_entry(
                report,
                path,
                &field,
                m,
                Shape {
                    require_by: true,
                    require_at: true,
                    verified: false,
                },
                ctx.roster(),
            );
        } else if m.contains_key(Yaml::String("by".into())) {
            check_entry(
                report,
                path,
                &field,
                m,
                Shape {
                    require_by: false,
                    require_at: false,
                    verified: false,
                },
                ctx.roster(),
            );
        }
        let live: HashSet<String> = page
            .frontmatter
            .strings(&key)
            .iter()
            .map(|v| normal(v))
            .collect();
        let present = live.contains(&normal(&value));
        match (closed, present) {
            (false, false) => report.push(
                path,
                Severity::Error,
                "STATEMENT_MISMATCH",
                format!("`{field}` is open, but `{value}` is not in the live `{key}` list"),
            ),
            (true, true) => report.push(
                path,
                Severity::Error,
                "STATEMENT_MISMATCH",
                format!("`{field}` is closed, but `{value}` is still in the live `{key}` list"),
            ),
            _ => {}
        }
        if let Some(Yaml::Sequence(ids)) = m.get(Yaml::String("sources".into())) {
            for id in ids.iter().filter_map(Yaml::as_str) {
                if !source_ids.contains(id) {
                    report.push(
                        path,
                        Severity::Warning,
                        "UNKNOWN_SOURCE_ID",
                        format!("`{field}` cites source `{id}`, which the page does not declare"),
                    );
                }
            }
        }
    }
}

/// Vault-wide temporal rules: supersession cycles and relations that point at
/// deprecated pages.
pub(crate) fn check_corpus(
    report: &mut Report,
    vault: &Vault,
    vocab: &Vocabulary,
    ctx: &Context<'_>,
) {
    cycles(report, vault, ctx);
    superseded_targets(report, vault, vocab, ctx);
}

/// `SUPERSESSION_CYCLE`: an edge runs from the page that replaces to the page
/// it replaces, written either as `supersedes` on the replacement or as
/// `superseded-by` on the replaced. A withdrawal entry has no `resource` and
/// so no edge.
fn cycles(report: &mut Report, vault: &Vault, ctx: &Context<'_>) {
    let index: HashMap<&str, usize> = vault
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); vault.pages.len()];
    let node = |reference: &str| -> Option<usize> {
        ctx.resolve(reference)
            .and_then(|t| (t.kind == vault.kind).then(|| index.get(t.page.id.as_str()).copied()))
            .flatten()
    };
    for (i, page) in vault.pages.iter().enumerate() {
        for entry in entries(page) {
            if let Some(j) = entry.resource.as_deref().and_then(node) {
                edges[i].push(j);
            }
        }
        for reference in superseded_by(page) {
            if let Some(j) = node(&reference) {
                edges[j].push(i);
            }
        }
    }
    // Iterative three-colour depth-first search; each ring is reported once,
    // on the page where the walk closes it.
    let mut colour = vec![0u8; edges.len()];
    for start in 0..edges.len() {
        if colour[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        colour[start] = 1;
        while let Some((n, k)) = stack.pop() {
            if let Some(&m) = edges[n].get(k) {
                stack.push((n, k + 1));
                match colour[m] {
                    0 => {
                        colour[m] = 1;
                        stack.push((m, 0));
                    }
                    1 => {
                        let mut ring: Vec<&str> = stack
                            .iter()
                            .map(|(p, _)| vault.pages[*p].id.as_str())
                            .skip_while(|id| *id != vault.pages[m].id)
                            .collect();
                        ring.dedup();
                        ring.push(vault.pages[m].id.as_str());
                        report.push(
                            &vault.pages[n].id,
                            Severity::Error,
                            "SUPERSESSION_CYCLE",
                            format!("supersession forms a ring: {}", ring.join(" -> ")),
                        );
                    }
                    _ => {}
                }
            } else {
                colour[n] = 2;
            }
        }
    }
}

fn superseded_targets(report: &mut Report, vault: &Vault, vocab: &Vocabulary, ctx: &Context<'_>) {
    for page in &vault.pages {
        if page.frontmatter.text("status").as_deref() == Some("deprecated") {
            continue;
        }
        for key in vocab.relations.keys() {
            for link in page.frontmatter.wikilinks(key) {
                let Some(target) = ctx.resolve(&format!("[[{}]]", link.target)) else {
                    continue;
                };
                if target.page.frontmatter.text("status").as_deref() == Some("deprecated") {
                    report.push(
                        &page.id,
                        Severity::Error,
                        "SUPERSEDED_TARGET",
                        format!(
                            "`{key}` points at `{}`, which is deprecated; retarget it",
                            target.page.id
                        ),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::Siblings;
    use crate::check::validate::Report;
    use crate::input::page::VaultKind;

    const B4: &str = "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";

    fn k(extra: &str) -> String {
        format!("---\ntype: Class\npublic: true\nresource: urn:ngm:class:x\nstatus: draft\n{extra}---\n")
    }

    fn stamp(reason: &str, resource: &str) -> String {
        format!("supersedes:\n  - {{ resource: '{resource}', reason: {reason}, at: '2026-11-21T00:00:00Z', by: {B4} }}\n")
    }

    fn run_k(pages: &[(&str, String)]) -> Report {
        let refs: Vec<(&str, &str)> = pages.iter().map(|(i, t)| (*i, t.as_str())).collect();
        run(&refs, VaultKind::Knowledge, &vocab(3, ""))
    }

    fn n(r: &Report, code: &str) -> usize {
        r.issues.iter().filter(|i| i.code == code).count()
    }

    #[test]
    fn valid_time_must_be_a_utc_second_precision_datetime() {
        let ok = run_k(&[("A", k("valid-from: 2022-10-25T00:00:00Z\n"))]);
        assert!(!has(&ok, "INVALID_DATETIME"), "{:?}", codes(&ok));
        for bad in [
            "2022-10-25",
            "2022-10-25T00:00:00",
            "2022-10-25T00:00:00.5Z",
            "soon",
            "2022-10-25T00:00:00+01:00",
        ] {
            let r = run_k(&[("A", k(&format!("valid-from: '{bad}'\n")))]);
            assert!(has(&r, "INVALID_DATETIME"), "{bad}");
        }
    }

    #[test]
    fn an_empty_or_inverted_interval_is_flagged() {
        let r = run_k(&[("A", k("status: deprecated\nvalid-from: 2026-02-01T00:00:00Z\nvalid-to: 2026-01-01T00:00:00Z\nsuperseded-by: ['[[B]]']\n").replace("status: draft\n", ""))]);
        assert!(has(&r, "VALID_RANGE"), "{:?}", codes(&r));
    }

    #[test]
    fn valid_to_needs_deprecated_on_a_plain_page() {
        let bad = run_k(&[("A", k("valid-to: 2026-01-01T00:00:00Z\n"))]);
        assert!(has(&bad, "VALID_TO_STATUS"));
        let good = run_k(&[
            (
                "A",
                k("valid-to: 2026-01-01T00:00:00Z\n")
                    .replace("status: draft", "status: deprecated"),
            ),
            ("B", k("")),
        ]);
        assert!(!has(&good, "VALID_TO_STATUS"), "{:?}", codes(&good));
    }

    #[test]
    fn valid_to_on_a_memory_kind_needs_superseded_or_archived() {
        let page = |status: &str| {
            format!("---\ntype: Belief\nstatus: {status}\nvalid-to: 2026-01-01T00:00:00Z\n---\n")
        };
        for (status, ok) in [
            ("superseded", true),
            ("archived", true),
            ("approved", false),
            ("invalidated", false),
            ("deprecated", false),
        ] {
            let r = run(
                &[("Fact", &page(status))],
                VaultKind::Working,
                &vocab(3, ""),
            );
            assert_eq!(!has(&r, "VALID_TO_STATUS"), ok, "{status}: {:?}", codes(&r));
        }
    }

    #[test]
    fn a_supersedes_entry_has_a_reason_a_resource_a_principal_and_a_time() {
        let ok = run_k(&[
            ("A", k(&stamp("merged", "urn:ngm:class:old"))),
            ("B", k(&stamp("renamed", "[[A]]"))),
        ]);
        assert!(
            !has(&ok, "INVALID_SUPERSEDES")
                && !has(&ok, "INVALID_STAMP")
                && !has(&ok, "INVALID_DATETIME"),
            "{:?}",
            codes(&ok)
        );
        let bad_reason = run_k(&[("A", k(&stamp("deleted", "urn:ngm:class:old")))]);
        assert!(has(&bad_reason, "INVALID_SUPERSEDES"));
        let no_reason = run_k(&[("A", k(&format!("supersedes:\n  - {{ resource: 'urn:ngm:class:old', at: '2026-11-21T00:00:00Z', by: {B4} }}\n")))]);
        assert!(has(&no_reason, "INVALID_SUPERSEDES"));
        let no_by = run_k(&[("A", k("supersedes:\n  - { resource: 'urn:ngm:class:old', reason: merged, at: '2026-11-21T00:00:00Z' }\n"))]);
        assert!(has(&no_by, "INVALID_STAMP"));
        let bad_at = run_k(&[("A", k(&format!("supersedes:\n  - {{ resource: 'urn:ngm:class:old', reason: merged, at: yesterday, by: {B4} }}\n")))]);
        assert!(has(&bad_at, "INVALID_DATETIME"));
        let plain_string = run_k(&[("A", k("supersedes: [old]\n"))]);
        assert!(has(&plain_string, "INVALID_SUPERSEDES"));
        let not_list = run_k(&[("A", k("supersedes: old\n"))]);
        assert!(has(&not_list, "INVALID_SUPERSEDES"));
    }

    #[test]
    fn a_knowledge_resource_is_a_wikilink_or_an_iri_never_bare_text() {
        let bare = run_k(&[("A", k(&stamp("merged", "Decentralised Finance")))]);
        assert!(has(&bare, "INVALID_SUPERSEDES"), "{:?}", codes(&bare));
        let urn = run_k(&[("A", k(&stamp("merged", "urn:visionflow:page:abc")))]);
        assert!(
            !has(&urn, "INVALID_SUPERSEDES"),
            "an IRI is legal in knowledge"
        );
    }

    #[test]
    fn a_working_resource_is_the_page_resource_urn() {
        let page = |resource: &str| {
            format!("---\ntype: Belief\nstatus: draft\nsupersedes:\n  - {{ resource: '{resource}', reason: corrected, at: '2026-11-21T00:00:00Z', by: {B4} }}\n---\n")
        };
        let ok = run(
            &[("Fact", &page("urn:visionflow:page:0a1b2c"))],
            VaultKind::Working,
            &vocab(3, ""),
        );
        assert!(!has(&ok, "INVALID_SUPERSEDES"), "{:?}", codes(&ok));
        let link = run(
            &[("Fact", &page("[[Other]]"))],
            VaultKind::Working,
            &vocab(3, ""),
        );
        assert!(has(&link, "INVALID_SUPERSEDES"));
        let https = run(
            &[("Fact", &page("https://example.org/x"))],
            VaultKind::Working,
            &vocab(3, ""),
        );
        assert!(has(&https, "INVALID_SUPERSEDES"));
    }

    #[test]
    fn a_withdrawal_entry_has_no_resource_and_only_corrected_or_withdrawn() {
        let entry = |reason: &str| {
            k(&format!("status: deprecated\nsupersedes:\n  - {{ reason: {reason}, at: '2026-11-21T00:00:00Z', by: {B4} }}\n")).replace("status: draft\n", "")
        };
        for reason in ["withdrawn", "corrected"] {
            let r = run_k(&[("A", entry(reason))]);
            assert!(
                !has(&r, "INVALID_SUPERSEDES") && !has(&r, "DEPRECATED_NO_SUCCESSOR"),
                "{reason}: {:?}",
                codes(&r)
            );
        }
        let r = run_k(&[("A", entry("merged"))]);
        assert!(has(&r, "INVALID_SUPERSEDES"));
        // A withdrawal entry names no page, so it is no edge and no cycle.
        assert!(!has(&r, "SUPERSESSION_CYCLE"));
    }

    #[test]
    fn deprecated_needs_a_successor_or_its_own_withdrawal() {
        let bare = run_k(&[("A", k("").replace("draft", "deprecated"))]);
        assert!(has(&bare, "DEPRECATED_NO_SUCCESSOR"));
        let replaced = run_k(&[
            (
                "A",
                k("superseded-by: ['[[B]]']\n").replace("draft", "deprecated"),
            ),
            ("B", k("")),
        ]);
        assert!(
            !has(&replaced, "DEPRECATED_NO_SUCCESSOR"),
            "{:?}",
            codes(&replaced)
        );
    }

    #[test]
    fn world_changed_expects_the_replaced_page_to_close_valid_time() {
        let no_clock = run_k(&[
            ("Old", k("")),
            ("New", k(&stamp("world-changed", "[[Old]]"))),
        ]);
        assert!(
            has(&no_clock, "SUPERSESSION_CLOCK"),
            "{:?}",
            codes(&no_clock)
        );
        let closed = run_k(&[
            (
                "Old",
                k("valid-to: 2026-11-21T00:00:00Z\nsuperseded-by: ['[[New]]']\n")
                    .replace("draft", "deprecated"),
            ),
            ("New", k(&stamp("world-changed", "[[Old]]"))),
        ]);
        assert!(!has(&closed, "SUPERSESSION_CLOCK"), "{:?}", codes(&closed));
        // `corrected` closes recorded time, `merged` closes neither: no valid-to expected.
        for reason in ["corrected", "merged", "renamed", "withdrawn"] {
            let r = run_k(&[("Old", k("")), ("New", k(&stamp(reason, "[[Old]]")))]);
            assert!(!has(&r, "SUPERSESSION_CLOCK"), "{reason}");
        }
    }

    #[test]
    fn a_dangling_wikilink_warns_and_a_folded_iri_does_not() {
        let r = run_k(&[("A", k(&stamp("merged", "[[Nowhere]]")))]);
        assert_eq!(n(&r, "DANGLING_SUPERSESSION"), 1);
        let folded = run_k(&[("A", k(&stamp("merged", "urn:ngm:class:folded-away")))]);
        assert!(!has(&folded, "DANGLING_SUPERSESSION"));
        let by = run_k(&[(
            "A",
            k("superseded-by: ['[[Nowhere]]']\n").replace("draft", "deprecated"),
        )]);
        assert!(has(&by, "DANGLING_SUPERSESSION"));
    }

    #[test]
    fn supersession_rings_are_errors_however_they_are_written() {
        let two = run_k(&[
            ("A", k(&stamp("merged", "[[B]]"))),
            ("B", k(&stamp("merged", "[[A]]"))),
        ]);
        assert!(has(&two, "SUPERSESSION_CYCLE"), "{:?}", codes(&two));
        let mixed = run_k(&[
            ("A", k(&stamp("merged", "[[B]]"))),
            (
                "B",
                k("superseded-by: ['[[B]]']\n").replace("draft", "deprecated"),
            ),
        ]);
        // B is superseded by itself: an edge B -> B.
        assert!(has(&mixed, "SUPERSESSION_CYCLE"), "{:?}", codes(&mixed));
        let three = run_k(&[
            ("A", k(&stamp("merged", "[[B]]"))),
            ("B", k(&stamp("merged", "[[C]]"))),
            ("C", k(&stamp("merged", "[[A]]"))),
        ]);
        assert!(has(&three, "SUPERSESSION_CYCLE"));
        let chain = run_k(&[
            ("A", k(&stamp("merged", "[[B]]"))),
            ("B", k(&stamp("merged", "[[C]]"))),
            ("C", k("")),
        ]);
        assert!(!has(&chain, "SUPERSESSION_CYCLE"), "{:?}", codes(&chain));
    }

    #[test]
    fn a_live_relation_to_a_deprecated_page_is_a_superseded_target() {
        let r = run_k(&[
            ("Live", k("requires: ['[[Gone]]']\n")),
            (
                "Gone",
                k("superseded-by: ['[[Live]]']\n").replace("draft", "deprecated"),
            ),
        ]);
        assert_eq!(n(&r, "SUPERSEDED_TARGET"), 1, "{:?}", codes(&r));
        // A deprecated page may still point at one; it is not live.
        let both = run_k(&[
            (
                "Gone",
                k("requires: ['[[Gone2]]']\nsuperseded-by: ['[[Live]]']\n")
                    .replace("draft", "deprecated"),
            ),
            (
                "Gone2",
                k("superseded-by: ['[[Live]]']\n").replace("draft", "deprecated"),
            ),
            ("Live", k("")),
        ]);
        assert!(!has(&both, "SUPERSEDED_TARGET"), "{:?}", codes(&both));
    }

    const STATEMENTS: &str = "requires: ['[[ISO 27001 2022]]']\nstatements:\n  - { key: requires, value: '[[ISO 27001 2022]]', valid-from: 2022-10-25T00:00:00Z }\n  - { key: requires, value: '[[ISO 27001 2013]]', valid-from: 2013-10-01T00:00:00Z, valid-to: 2022-10-25T00:00:00Z, reason: world-changed, replaced-by: '[[ISO 27001 2022]]', at: '2026-11-21T10:00:00Z', by: BY, sources: [iso] }\nsources:\n  - { id: iso, resource: 'https://www.iso.org/standard/27001' }\n";

    fn statements(extra: &str) -> String {
        k(&format!("{STATEMENTS}{extra}")).replace("BY", B4)
    }

    #[test]
    fn statements_match_the_live_relation_lists() {
        let r = run_k(&[("A", statements(""))]);
        assert!(
            !has(&r, "STATEMENT_MISMATCH")
                && !has(&r, "INVALID_STATEMENT")
                && !has(&r, "UNKNOWN_SOURCE_ID"),
            "{:?}",
            codes(&r)
        );
        // An open statement that is not in the list.
        let open_missing = k("requires: ['[[Other]]']\nstatements:\n  - { key: requires, value: '[[ISO 27001 2022]]' }\n");
        assert!(has(&run_k(&[("A", open_missing)]), "STATEMENT_MISMATCH"));
        // A closed statement whose value is still live.
        let closed_live = k(&format!("requires: ['[[ISO 27001 2013]]']\nstatements:\n  - {{ key: requires, value: '[[ISO 27001 2013]]', valid-to: 2022-10-25T00:00:00Z, reason: world-changed, at: '2026-11-21T10:00:00Z', by: {B4} }}\n"));
        assert!(has(&run_k(&[("A", closed_live)]), "STATEMENT_MISMATCH"));
    }

    #[test]
    fn statement_entries_follow_the_grammar() {
        let bad_key = k("statements:\n  - { key: not-a-relation, value: x }\n");
        assert!(has(&run_k(&[("A", bad_key)]), "INVALID_STATEMENT"));
        let no_value = k("requires: ['[[X]]']\nstatements:\n  - { key: requires }\n");
        assert!(has(&run_k(&[("A", no_value)]), "INVALID_STATEMENT"));
        let not_list = k("statements: nope\n");
        assert!(has(&run_k(&[("A", not_list)]), "INVALID_STATEMENT"));
        let bad_reason = k(&format!("requires: ['[[X]]']\nstatements:\n  - {{ key: requires, value: '[[Y]]', valid-to: 2022-10-25T00:00:00Z, reason: whim, at: '2026-11-21T10:00:00Z', by: {B4} }}\n"));
        assert!(has(&run_k(&[("A", bad_reason)]), "INVALID_STATEMENT"));
        // A closed statement without who and when.
        let anonymous = k("requires: ['[[X]]']\nstatements:\n  - { key: requires, value: '[[Y]]', valid-to: 2022-10-25T00:00:00Z, reason: world-changed }\n");
        assert!(has(&run_k(&[("A", anonymous)]), "INVALID_STAMP"));
        let bad_time = k("requires: ['[[X]]']\nstatements:\n  - { key: requires, value: '[[X]]', valid-from: 2022 }\n");
        assert!(has(&run_k(&[("A", bad_time)]), "INVALID_DATETIME"));
        let ghost_source = k("requires: ['[[X]]']\nstatements:\n  - { key: requires, value: '[[X]]', sources: [nope] }\n");
        assert!(has(&run_k(&[("A", ghost_source)]), "UNKNOWN_SOURCE_ID"));
    }

    #[test]
    fn version_2_ignores_the_whole_family() {
        let page = k("valid-from: soon\nsupersedes: nope\nstatements: nope\n");
        let vault = vault_of(&[("A", &page)], VaultKind::Knowledge);
        let r = run_with(&vault, &vocab(2, ""), &Siblings::default());
        for code in [
            "INVALID_DATETIME",
            "INVALID_SUPERSEDES",
            "INVALID_STATEMENT",
        ] {
            assert!(!has(&r, code), "{code}");
        }
    }
}
