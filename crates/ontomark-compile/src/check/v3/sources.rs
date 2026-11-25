//! Typed sources and the working-memory kinds' own keys (ADR-VG-010 §5-§6,
//! rulings B7 and B12).
//!
//! | code | severity | rule |
//! |---|---|---|
//! | `INVALID_SOURCE` | error | a `sources` entry without `id`, a resolving `rel` without `resource`, or an unknown `rel` |
//! | `INVALID_SOURCE_RESOURCE` | error | `origin`, `episode`, `evidence` or `derived-from` whose `resource` is free text |
//! | `DANGLING_SOURCE` | error | such a wikilink names no page, when both vaults are loaded |
//! | `BRIEF_AS_EVIDENCE` | error | `rel: evidence` or `origin` pointing at a `Brief` |
//! | `MISSING_LOCATOR` | error | `rel: evidence` to an `Episode` ledger without a `locator` |
//! | `DANGLING_LOCATOR` | error | a `fp:` fingerprint absent from the ledger it cites |
//! | `INVALID_PIN` | error | a `sha256-v1:` pin that is not 64 lowercase hex, or the retired `sha256:` spelling |
//! | `STALE_PIN` | warning | a `derived-from` pin that no longer matches the target's digest |
//! | `MISSING_KIND_KEY` | error | a memory kind without one of its required keys |
//! | `MISSING_EVIDENCE` | error | a `Proposal`, `Belief` or `Procedure` with no `evidence` source carrying a locator |
//! | `MISSING_ORIGIN` | warning | an `Episode` with no `rel: origin` source (B7: a warning until the migration lists no unmatched ledgers) |
//! | `INVALID_KIND_VALUE` | error | `scope`, `about-kind`, `confidence` or `topic` outside its domain |
//!
//! Pins are ontomark's own `sha256-v1:` page digest
//! ([`ontomark_parse::certification::content_digest`]), not a new hash.

use std::collections::HashSet;

use ontomark_parse::certification::content_digest;
use ontomark_parse::Document;
use serde_norway::Value as Yaml;

use crate::check::validate::{Report, Severity};
use crate::input::frontmatter::Wikilink;
use crate::input::page::{Page, VaultKind};
use crate::input::vocabulary::Vocabulary;

use super::temporal::is_iri;
use super::Context;

/// The six `rel` values (ADR-VG-010 §5).
pub const RELS: [&str; 6] = [
    "origin",
    "episode",
    "evidence",
    "derived-from",
    "standard",
    "reference",
];

/// The `rel` values whose `resource` must resolve.
const RESOLVING: [&str; 4] = ["origin", "episode", "evidence", "derived-from"];

/// The closed `about-kind` set (ADR-VG-010 §6).
pub const ABOUT_KINDS: [&str; 8] = [
    "person",
    "organisation",
    "product",
    "project",
    "place",
    "event",
    "document",
    "concept",
];

/// The assertion fingerprints a ledger body carries, as lowercase hex.
#[must_use]
pub fn fingerprints(body: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut rest = body;
    while let Some(i) = rest.find("assertion-fp:") {
        rest = &rest[i + "assertion-fp:".len()..];
        let hex: String = rest
            .trim_start()
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        if !hex.is_empty() {
            out.insert(hex.to_ascii_lowercase());
        }
    }
    out
}

/// The `sha256-v1:` digest of a page as ontomark pins it: the whole page,
/// `verified` excluded.
#[must_use]
pub fn page_pin(page: &Page) -> Option<String> {
    content_digest(&Document::parse(page.source.clone()), "verified", None)
}

fn err(report: &mut Report, path: &str, code: &str, message: String) {
    report.push(path, Severity::Error, code, message);
}

/// Per-page source and kind rules.
pub(crate) fn check_page(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    let path = page.id.as_str();
    let ty = page.frontmatter.text("type").unwrap_or_default();
    let mut evidence_with_locator = false;
    let mut has_origin = false;

    if let Some(value) = page.frontmatter.get("sources") {
        let Yaml::Sequence(items) = value else {
            err(
                report,
                path,
                "INVALID_SOURCE",
                "`sources` must be a list".into(),
            );
            return;
        };
        for (i, item) in items.iter().enumerate() {
            let field = format!("sources[{i}]");
            let m = item.as_mapping();
            let text = |k: &str| {
                m.and_then(|m| m.get(Yaml::String(k.into())))
                    .and_then(Yaml::as_str)
                    .map(str::to_owned)
            };
            if text("id").is_none() {
                err(
                    report,
                    path,
                    "INVALID_SOURCE",
                    format!("`{field}` needs a text `id`"),
                );
                continue;
            }
            let rel = text("rel").unwrap_or_else(|| "reference".into());
            if !RELS.contains(&rel.as_str()) {
                err(
                    report,
                    path,
                    "INVALID_SOURCE",
                    format!("`{field}`: `rel: {rel}` is not one of {}", RELS.join(", ")),
                );
                continue;
            }
            // ADR-VG-010 §5: `resource` must resolve for the four resolving
            // relations; a plain reference (the default) may be an id alone,
            // as 6 knowledge pages have carried since v2.
            let Some(resource) = text("resource") else {
                if RESOLVING.contains(&rel.as_str()) {
                    err(
                        report,
                        path,
                        "INVALID_SOURCE",
                        format!("`{field}`: `rel: {rel}` needs a `resource`"),
                    );
                }
                continue;
            };
            if let Some(at) = m.and_then(|m| m.get(Yaml::String("at".into()))) {
                if at.as_str().is_none_or(|s| s.trim().is_empty()) {
                    err(
                        report,
                        path,
                        "INVALID_DATETIME",
                        format!("`{field}.at` must be a date or date-time"),
                    );
                }
            }
            let locator = text("locator");
            if rel == "origin" {
                has_origin = true;
            }
            if rel == "evidence" && locator.is_some() {
                evidence_with_locator = true;
            }
            let target = Wikilink::parse(&resource).and_then(|_| ctx.resolve(&resource));
            if RESOLVING.contains(&rel.as_str()) {
                let link = Wikilink::parse(&resource).is_some();
                if !link && !is_iri(&resource) {
                    err(
                        report,
                        path,
                        "INVALID_SOURCE_RESOURCE",
                        format!(
                            "`{field}`: `rel: {rel}` needs a wikilink, URL or IRI, not free text"
                        ),
                    );
                } else if link
                    && target.is_none()
                    && ctx.sees(VaultKind::Knowledge)
                    && ctx.sees(VaultKind::Working)
                {
                    report.push(
                        path,
                        Severity::Error,
                        "DANGLING_SOURCE",
                        format!("`{field}`: `{resource}` names no page"),
                    );
                }
            }
            let Some(target) = target else { continue };
            let target_type = target.page.frontmatter.text("type").unwrap_or_default();
            if target_type == "Brief" && matches!(rel.as_str(), "evidence" | "origin") {
                err(
                    report,
                    path,
                    "BRIEF_AS_EVIDENCE",
                    format!(
                        "`{field}`: `{}` is a Brief, and briefs are not evidence",
                        target.page.id
                    ),
                );
            }
            if target_type == "Episode" {
                check_ledger_locator(report, path, &field, &rel, locator.as_deref(), target.page);
            }
            if rel == "derived-from" {
                if let Some(locator) = &locator {
                    check_pins(report, path, &field, locator, target.page);
                }
            }
        }
    }
    kind_rules(
        report,
        page,
        &ty,
        vocab,
        ctx,
        has_origin,
        evidence_with_locator,
    );
}

fn check_ledger_locator(
    report: &mut Report,
    path: &str,
    field: &str,
    rel: &str,
    locator: Option<&str>,
    ledger: &Page,
) {
    let Some(locator) = locator else {
        if rel == "evidence" {
            err(
                report,
                path,
                "MISSING_LOCATOR",
                format!("`{field}`: evidence in a ledger needs `locator: fp:<assertion-fp>`"),
            );
        }
        return;
    };
    let known = fingerprints(&ledger.body);
    for part in locator.split(',').map(str::trim) {
        if let Some(fp) = part.strip_prefix("fp:") {
            if !known.contains(&fp.trim().to_ascii_lowercase()) {
                err(
                    report,
                    path,
                    "DANGLING_LOCATOR",
                    format!("`{field}`: fingerprint {fp} is not in `{}`", ledger.id),
                );
            }
        }
    }
}

fn check_pins(report: &mut Report, path: &str, field: &str, locator: &str, target: &Page) {
    for part in locator.split(',').map(str::trim) {
        if part.starts_with("sha256:") {
            err(
                report,
                path,
                "INVALID_PIN",
                format!("`{field}`: pins use ontomark's `sha256-v1:` form, not `sha256:`"),
            );
        } else if let Some(hex) = part.strip_prefix("sha256-v1:") {
            let ok = hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
            if !ok {
                err(
                    report,
                    path,
                    "INVALID_PIN",
                    format!("`{field}`: `{part}` is not 64 lowercase hex"),
                );
            } else if page_pin(target).is_some_and(|d| d != part) {
                report.push(
                    path,
                    Severity::Warning,
                    "STALE_PIN",
                    format!("`{field}`: `{}` has changed since it was pinned", target.id),
                );
            }
        }
    }
}

fn kind_rules(
    report: &mut Report,
    page: &Page,
    ty: &str,
    vocab: &Vocabulary,
    ctx: &Context<'_>,
    has_origin: bool,
    evidence_with_locator: bool,
) {
    if ctx.vault.kind != VaultKind::Working {
        return;
    }
    let path = page.id.as_str();
    let fm = &page.frontmatter;
    let required: &[&str] = match ty {
        "Episode" => &["episode-url", "episode-date"],
        "Brief" => &["scope"],
        "Proposal" => &["proposes", "topic"],
        "Belief" => &["about", "about-kind", "confidence"],
        "Procedure" => &["about"],
        _ => &[],
    };
    // `about-kind` is optional (ADR-VG-010 §6): listed here only to be
    // value-checked below.
    for key in required.iter().filter(|k| **k != "about-kind") {
        if !fm.has(key) {
            err(
                report,
                path,
                "MISSING_KIND_KEY",
                format!("`type: {ty}` requires `{key}`"),
            );
        }
    }
    let _ = vocab;
    if ty == "Episode" && !has_origin {
        report.push(
            path,
            Severity::Warning,
            "MISSING_ORIGIN",
            "an Episode names its recording with a `rel: origin` source",
        );
    }
    if matches!(ty, "Proposal" | "Belief" | "Procedure") && !evidence_with_locator {
        err(
            report,
            path,
            "MISSING_EVIDENCE",
            format!("`type: {ty}` needs at least one `rel: evidence` source with a `locator`"),
        );
    }
    let bad =
        |report: &mut Report, message: String| err(report, path, "INVALID_KIND_VALUE", message);
    if let Some(scope) = fm.text("scope").filter(|_| ty == "Brief") {
        if !matches!(scope.as_str(), "session" | "topic") {
            bad(report, format!("`scope: {scope}` is not session or topic"));
        }
    }
    if ty == "Brief" || ty == "Proposal" {
        if let Some(topic) = fm.text("topic") {
            if Wikilink::parse(&topic).is_some() {
                bad(report, "`topic` is a free label, never a link".into());
            }
        }
    }
    if let Some(kind) = fm
        .text("about-kind")
        .filter(|_| matches!(ty, "Belief" | "Procedure"))
    {
        if !ABOUT_KINDS.contains(&kind.as_str()) {
            bad(
                report,
                format!(
                    "`about-kind: {kind}` is not one of {}",
                    ABOUT_KINDS.join(", ")
                ),
            );
        }
    }
    if ty == "Belief" {
        if let Some(c) = fm.get("confidence") {
            if !c.as_f64().is_some_and(|c| (0.0..=1.0).contains(&c)) {
                bad(report, "`confidence` must be a number from 0 to 1".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::check::validate::Report;
    use crate::input::page::VaultKind;

    const LEDGER: &str = "---\ntype: Episode\nstatus: approved\nepisode-url: https://x.example/e\nepisode-date: 2026-05-01\nsources:\n  - { id: o, resource: 'https://x.example/e', rel: origin }\n---\n- A claim.\n  <!-- assertion-fp: db4adc4ad5eafef4 -->\n";

    fn w(pages: &[(&str, &str)]) -> Report {
        run(pages, VaultKind::Working, &vocab(3, ""))
    }

    fn belief(sources: &str) -> String {
        format!("---\ntype: Belief\nstatus: draft\nabout: '[[X]]'\nconfidence: 0.9\nsources:\n{sources}---\n")
    }

    #[test]
    fn fingerprints_are_read_from_comments() {
        let fps = fingerprints(LEDGER);
        assert!(fps.contains("db4adc4ad5eafef4") && fps.len() == 1);
    }

    #[test]
    fn rel_must_be_known_and_resolving_rels_refuse_free_text() {
        let bad_rel = w(&[(
            "B",
            &belief("  - { id: a, resource: 'https://x.example', rel: gossip }\n"),
        )]);
        assert!(has(&bad_rel, "INVALID_SOURCE"));
        let free = w(&[(
            "B",
            &belief("  - { id: a, resource: 'AI Daily Brief', rel: evidence, locator: 'fp:aa' }\n"),
        )]);
        assert!(has(&free, "INVALID_SOURCE_RESOURCE"));
        let reference = w(&[("B", &belief("  - { id: a, resource: 'AI Daily Brief' }\n  - { id: b, resource: 'ISO 27001', rel: standard }\n"))]);
        assert!(
            !has(&reference, "INVALID_SOURCE_RESOURCE"),
            "{:?}",
            codes(&reference)
        );
        let no_id = w(&[("B", &belief("  - { resource: 'https://x.example' }\n"))]);
        assert!(has(&no_id, "INVALID_SOURCE"));
        // An id-only plain reference is legal (ADR-VG-010 §5); a resolving rel needs a resource.
        let id_only = w(&[("B", &belief("  - { id: origin }\n"))]);
        assert!(!has(&id_only, "INVALID_SOURCE"), "{:?}", codes(&id_only));
        let origin_bare = w(&[("B", &belief("  - { id: origin, rel: origin }\n"))]);
        assert!(has(&origin_bare, "INVALID_SOURCE"));
    }

    #[test]
    fn a_brief_is_not_evidence_but_may_be_derived_from_or_referenced() {
        let brief = "---\ntype: Brief\nstatus: draft\nscope: topic\n---\nbody\n";
        for (rel, flagged) in [
            ("evidence", true),
            ("origin", true),
            ("derived-from", false),
            ("reference", false),
        ] {
            let src =
                format!("  - {{ id: a, resource: '[[Digest]]', rel: {rel}, locator: 'fp:aa' }}\n");
            let r = w(&[("Digest", brief), ("B", &belief(&src))]);
            assert_eq!(has(&r, "BRIEF_AS_EVIDENCE"), flagged, "{rel}");
        }
    }

    #[test]
    fn evidence_in_a_ledger_needs_a_locator_whose_fingerprints_exist() {
        let ok = w(&[("L", LEDGER), ("B", &belief("  - { id: a, resource: '[[L]]', rel: evidence, locator: 'fp:db4adc4ad5eafef4' }\n"))]);
        assert!(
            !has(&ok, "DANGLING_LOCATOR")
                && !has(&ok, "MISSING_LOCATOR")
                && !has(&ok, "MISSING_EVIDENCE"),
            "{:?}",
            codes(&ok)
        );
        let missing = w(&[
            ("L", LEDGER),
            (
                "B",
                &belief("  - { id: a, resource: '[[L]]', rel: evidence }\n"),
            ),
        ]);
        assert!(has(&missing, "MISSING_LOCATOR") && has(&missing, "MISSING_EVIDENCE"));
        let dangling = w(&[("L", LEDGER), ("B", &belief("  - { id: a, resource: '[[L]]', rel: evidence, locator: 'fp:db4adc4ad5eafef4, fp:0000000000000000' }\n"))]);
        assert_eq!(
            dangling
                .issues
                .iter()
                .filter(|i| i.code == "DANGLING_LOCATOR")
                .count(),
            1
        );
    }

    #[test]
    fn pins_use_sha256_v1_and_a_stale_one_is_a_warning() {
        let brief = |pin: &str| {
            format!("---\ntype: Brief\nstatus: draft\nscope: topic\nsources:\n  - {{ id: a, resource: '[[L]]', rel: derived-from, locator: '{pin}' }}\n---\nb\n")
        };
        let page = crate::input::page::Page::parse("/v/L.md", "L.md", "L", LEDGER).unwrap();
        let pin = page_pin(&page).unwrap();
        assert!(pin.starts_with("sha256-v1:") && pin.len() == 10 + 64);
        let fresh = w(&[("L", LEDGER), ("D", &brief(&pin))]);
        assert!(
            !has(&fresh, "STALE_PIN") && !has(&fresh, "INVALID_PIN"),
            "{:?}",
            codes(&fresh)
        );
        let stale = w(&[
            ("L", LEDGER),
            ("D", &brief(&format!("sha256-v1:{}", "0".repeat(64)))),
        ]);
        assert!(has(&stale, "STALE_PIN"));
        assert!(has(
            &w(&[("L", LEDGER), ("D", &brief("sha256:abc"))]),
            "INVALID_PIN"
        ));
        assert!(has(
            &w(&[("L", LEDGER), ("D", &brief("sha256-v1:XYZ"))]),
            "INVALID_PIN"
        ));
    }

    #[test]
    fn a_dangling_source_link_warns_only_when_both_vaults_are_loaded() {
        let page =
            belief("  - { id: a, resource: '[[Nowhere]]', rel: evidence, locator: 'fp:aa' }\n");
        assert!(
            !has(&w(&[("B", &page)]), "DANGLING_SOURCE"),
            "one vault loaded: skip"
        );
        let know = vault_of(&[("K", "---\ntype: Class\n---\n")], VaultKind::Knowledge);
        let work = vault_of(&[("B", &page)], VaultKind::Working);
        let r = run_with(
            &work,
            &vocab(3, ""),
            &super::super::Siblings {
                vaults: vec![&know],
                roster: None,
            },
        );
        // Ruling 10: an error under vocabulary 3.
        assert!(
            r.errors().iter().any(|i| i.code == "DANGLING_SOURCE"),
            "{:?}",
            codes(&r)
        );
    }

    #[test]
    fn kinds_require_their_keys() {
        let r = w(&[("E", "---\ntype: Episode\nstatus: approved\n---\nx\n")]);
        assert_eq!(
            r.issues
                .iter()
                .filter(|i| i.code == "MISSING_KIND_KEY")
                .count(),
            2
        );
        assert!(has(&r, "MISSING_ORIGIN"));
        assert!(
            r.issues
                .iter()
                .find(|i| i.code == "MISSING_ORIGIN")
                .unwrap()
                .severity
                == crate::check::validate::Severity::Warning
        );
        assert!(!has(&w(&[("L", LEDGER)]), "MISSING_ORIGIN"));
        let p = w(&[("P", "---\ntype: Proposal\nstatus: draft\n---\nx\n")]);
        assert!(has(&p, "MISSING_KIND_KEY") && has(&p, "MISSING_EVIDENCE"));
        let proc_ = w(&[("R", "---\ntype: Procedure\nstatus: draft\n---\nx\n")]);
        assert!(has(&proc_, "MISSING_KIND_KEY"));
        let brief = w(&[("D", "---\ntype: Brief\nstatus: draft\n---\nx\n")]);
        assert!(has(&brief, "MISSING_KIND_KEY"));
    }

    #[test]
    fn kind_values_are_closed() {
        let ok_src = "  - { id: a, resource: 'https://x.example', rel: evidence, locator: 'p4' }\n";
        let page = |extra: &str| {
            format!(
                "---\ntype: Belief\nstatus: draft\nabout: '[[X]]'\n{extra}sources:\n{ok_src}---\n"
            )
        };
        assert!(!has(
            &w(&[("B", &page("confidence: 0.5\nabout-kind: person\n"))]),
            "INVALID_KIND_VALUE"
        ));
        for bad in [
            "confidence: 1.5\n",
            "confidence: high\n",
            "confidence: 0.5\nabout-kind: robot\n",
        ] {
            assert!(has(&w(&[("B", &page(bad))]), "INVALID_KIND_VALUE"), "{bad}");
        }
        assert!(has(
            &w(&[("D", "---\ntype: Brief\nstatus: draft\nscope: week\n---\n")]),
            "INVALID_KIND_VALUE"
        ));
        assert!(has(
            &w(&[(
                "D",
                "---\ntype: Brief\nstatus: draft\nscope: topic\ntopic: '[[Data]]'\n---\n"
            )]),
            "INVALID_KIND_VALUE"
        ));
    }

    #[test]
    fn version_2_ignores_sources_and_kinds() {
        let r = run(
            &[(
                "B",
                &belief("  - { id: a, resource: 'free text', rel: gossip }\n"),
            )],
            VaultKind::Working,
            &vocab(2, ""),
        );
        assert!(!has(&r, "INVALID_SOURCE") && !has(&r, "INVALID_SOURCE_RESOURCE"));
    }
}
