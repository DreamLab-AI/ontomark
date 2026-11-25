//! Stamps (ADR-VG-011, ADR-VG-012 §6): who is answerable (`by`, a principal
//! key), what did the work (`via`), and when (`at`).
//!
//! The same shape is read on `generated`, `verified[]` and `contributors[]`
//! here, and on `supersedes[]` and `statements[]` entries by `check_entry`.
//!
//! Both grammars parse under either vocabulary version. Strictness does not:
//!
//! * **Version 2.** A `generated` or `verified` value the tool cannot read as a
//!   stamp is reported as the warning `INVALID_STAMP`. It used to be dropped
//!   without a word.
//! * **Version 3.** The same defect is an error, and the stamp must also follow
//!   the v3 grammar: `by` names a principal (a `did:nostr:` with a 64 lowercase
//!   hex body; the `human:` spelling of vocabulary 2 is an error, ruling 2), `via` names an
//!   `agent:` or `process:` instrument, and `verified` entries are by humans
//!   according to the roster's principal `kind`.
//!
//! | code | version 2 | version 3 |
//! |---|---|---|
//! | `INVALID_STAMP` | warning | error |
//! | `NON_CANONICAL_PRINCIPAL` | — | warning |
//! | `VERIFIED_NOT_HUMAN` | — | error |
//! | `VERIFIED_PRINCIPAL_UNKNOWN` | — | warning |

use serde_norway::Value as Yaml;

use crate::build::attribution::{PrincipalKind, Roster};
use crate::check::validate::{Report, Severity};
use crate::input::okf::{Actor, Stamp};
use crate::input::page::Page;

use super::Context;

/// What an attribution mapping must carry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shape {
    /// `by` is required.
    pub require_by: bool,
    /// `at` is required.
    pub require_at: bool,
    /// The entry attests like a `verified` stamp: its principal must be human.
    pub verified: bool,
}

/// A stamp on `generated`, `verified[]` or `contributors[]`.
pub(crate) const STAMP: Shape = Shape {
    require_by: true,
    require_at: true,
    verified: false,
};

/// Version 2: report what the tool silently dropped.
pub(crate) fn legacy_warnings(report: &mut Report, page: &Page) {
    for (field, value) in stamp_values(page, false) {
        if Stamp::from_yaml(value).is_none() {
            report.push(
                &page.id,
                Severity::Warning,
                "INVALID_STAMP",
                format!(
                    "`{field}` is not a readable stamp ({}); it is ignored",
                    why_unreadable(value)
                ),
            );
        }
    }
}

/// Version 3: every stamp on the page follows the v3 grammar.
pub(crate) fn check_page(report: &mut Report, page: &Page, ctx: &Context<'_>) {
    let roster = ctx.roster();
    for (field, value) in stamp_values(page, true) {
        let verified = field.starts_with("verified");
        check_value(
            report,
            &page.id,
            &field,
            value,
            Shape { verified, ..STAMP },
            roster,
        );
    }
    if let Some(Yaml::Sequence(items)) = page.frontmatter.get("contributors") {
        for (i, item) in items.iter().enumerate() {
            check_value(
                report,
                &page.id,
                &format!("contributors[{i}]"),
                item,
                STAMP,
                roster,
            );
        }
    }
}

/// The `generated` and `verified[]` values of a page, named for messages.
/// A null `verified` (a bare `verified:`) is an absent key, not a stamp. A null
/// `generated` is kept when `keep_null_generated` (vocabulary 3): the page
/// claims a stamp and carries none, which is INVALID_STAMP, not silence.
fn stamp_values(page: &Page, keep_null_generated: bool) -> Vec<(String, &Yaml)> {
    let mut out = Vec::new();
    if let Some(v) = page
        .frontmatter
        .get("generated")
        .filter(|v| keep_null_generated || !v.is_null())
    {
        out.push(("generated".to_owned(), v));
    }
    match page.frontmatter.get("verified") {
        Some(Yaml::Sequence(items)) => {
            for (i, item) in items.iter().enumerate().filter(|(_, v)| !v.is_null()) {
                out.push((format!("verified[{i}]"), item));
            }
        }
        Some(v) if !v.is_null() => out.push(("verified".to_owned(), v)),
        _ => {}
    }
    out
}

/// Why [`Stamp::from_yaml`] declined `value`, in a phrase.
fn why_unreadable(value: &Yaml) -> String {
    let Some(m) = value.as_mapping() else {
        return "it is not a mapping".into();
    };
    let get = |key: &str| m.get(Yaml::String(key.into()));
    match get("by").and_then(Yaml::as_str) {
        None => "`by` is missing or not text".into(),
        Some(by) => match by.parse::<Actor>() {
            Err(e) => e,
            Ok(_) if get("at").and_then(Yaml::as_str).is_none() => {
                "`at` is missing or not text".into()
            }
            Ok(_) => "unreadable".into(),
        },
    }
}

/// Report `INVALID_STAMP` as an error.
fn invalid(report: &mut Report, path: &str, field: &str, message: impl std::fmt::Display) {
    report.push(
        path,
        Severity::Error,
        "INVALID_STAMP",
        format!("`{field}`: {message}"),
    );
}

/// Check one stamp-shaped value found at `field`.
pub(crate) fn check_value(
    report: &mut Report,
    path: &str,
    field: &str,
    value: &Yaml,
    shape: Shape,
    roster: Option<&Roster>,
) {
    let Some(m) = value.as_mapping() else {
        report.push(
            path,
            Severity::Error,
            "INVALID_STAMP",
            format!("`{field}` must be a mapping of `by`, `at` and optional `via`"),
        );
        return;
    };
    check_entry(report, path, field, m, shape, roster);
}

/// Check the `by`, `via` and `at` of an attribution mapping.
pub(crate) fn check_entry(
    report: &mut Report,
    path: &str,
    field: &str,
    m: &serde_norway::Mapping,
    shape: Shape,
    roster: Option<&Roster>,
) {
    let get = |key: &str| m.get(Yaml::String(key.into()));

    match get("by") {
        None if shape.require_by => invalid(report, path, field, "`by` is required"),
        None => {}
        Some(by) => match by.as_str().map(str::parse::<Actor>) {
            None => invalid(report, path, field, "`by` must be text"),
            Some(Err(e)) => invalid(report, path, field, e),
            Some(Ok(actor)) => {
                check_principal(report, path, field, &actor, shape, roster);
            }
        },
    }
    match get("at") {
        None if shape.require_at => invalid(report, path, field, "`at` is required"),
        Some(at) if at.as_str().is_none_or(|s| s.trim().is_empty()) => {
            invalid(report, path, field, "`at` must be a date-time");
        }
        _ => {}
    }
    if let Some(via) = get("via") {
        match via.as_str().map(str::parse::<Actor>) {
            Some(Ok(Actor::Agent { .. } | Actor::Process { .. })) => {}
            Some(Ok(other)) => invalid(
                report,
                path,
                field,
                format!("`via` names the instrument (agent: or process:), not {other}"),
            ),
            Some(Err(e)) => invalid(report, path, field, e),
            None => invalid(report, path, field, "`via` must be text"),
        }
    }
}

/// The principal rules for a parsed `by`.
fn check_principal(
    report: &mut Report,
    path: &str,
    field: &str,
    actor: &Actor,
    shape: Shape,
    roster: Option<&Roster>,
) {
    // Ruling 2 (LOCKED-PLAN, after S6): under vocabulary 3 `by` is the
    // principal's did:nostr, never the legacy `human:` spelling; m0002 rewrites
    // every such stamp.
    if let Actor::Human(name) = actor {
        invalid(
            report,
            path,
            field,
            format!(
                "`by: human:{name}` is the vocabulary 2 spelling; name the principal as did:nostr:<64 hex> (ontomark migrate m0002-vocabulary-v3 rewrites it)"
            ),
        );
        return;
    }
    let Some(did) = actor.principal_did() else {
        match actor {
            Actor::Human(name) => invalid(
                report,
                path,
                field,
                format!(
                    "`by: human:{name}` is not an npub; name the principal as did:nostr:<64 hex>"
                ),
            ),
            other => invalid(
                report,
                path,
                field,
                format!(
                    "`by` names the principal answerable, not the instrument {other}; \
                 put the agent or process in `via`"
                ),
            ),
        }
        return;
    };
    let Some(roster) = roster else {
        return;
    };
    match roster.resolve_principal(&did) {
        Some(resolved) => {
            if resolved.canonical != did {
                report.push(
                    path,
                    Severity::Warning,
                    "NON_CANONICAL_PRINCIPAL",
                    format!(
                        "`{field}`: {did} resolves to the canonical principal {}; write that",
                        resolved.canonical
                    ),
                );
            }
            if shape.verified && resolved.kind != PrincipalKind::Human {
                report.push(
                    path,
                    Severity::Error,
                    "VERIFIED_NOT_HUMAN",
                    format!(
                        "`{field}`: {} is not a human principal in the roster; \
                         only humans verify (ADR-VG-011 §4)",
                        resolved.canonical
                    ),
                );
            }
        }
        None if shape.verified => report.push(
            path,
            Severity::Warning,
            "VERIFIED_PRINCIPAL_UNKNOWN",
            format!("`{field}`: the roster does not list {did}, so it cannot be shown to be human"),
        ),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::Siblings;
    use crate::build::attribution::Roster;
    use crate::input::page::VaultKind;

    const B4: &str = "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a";
    const D5: &str = "did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246";
    const D5_NPUB: &str = "npub16kv2h3hdalphuyzj8tkycq8cm9n54ymer23558ds80afkc9e7frqwrawzj";
    const ORG: &str = "did:nostr:ea3d03d62a8eb3ce6364c0dd02f239a16fa22956a9f31626619f330361be9c00";

    fn roster() -> Roster {
        Roster::from_yaml_str(&format!(
            "principals:\n  - did: {B4}\n    label: jjohare\n    kind: human\n    also: [{D5}]\n  - did: {ORG}\n    label: acme\n    kind: organisation\n"
        ))
    }

    fn page(fm: &str) -> String {
        format!(
            "---\ntype: Class\nresource: urn:ngm:class:a\nstatus: draft\npublic: true\n{fm}---\n"
        )
    }

    fn run_k(fm: &str, version: u32, roster: Option<&Roster>) -> crate::check::validate::Report {
        let vault = vault_of(&[("A", &page(fm))], VaultKind::Knowledge);
        let siblings = Siblings {
            vaults: Vec::new(),
            roster,
        };
        run_with(&vault, &vocab(version, ""), &siblings)
    }

    fn count(r: &crate::check::validate::Report, code: &str) -> usize {
        r.issues.iter().filter(|i| i.code == code).count()
    }

    #[test]
    fn a_principal_stamp_with_a_via_is_clean() {
        let r = run_k(
            &format!("generated: {{ by: {B4}, via: 'agent:enrichment-swarm/6', at: '2026-09-22T12:41:39Z' }}\n"),
            3,
            None,
        );
        assert!(!has(&r, "INVALID_STAMP"), "{:?}", codes(&r));
    }

    #[test]
    fn a_non_hex_did_is_an_error_under_v3_and_a_warning_under_v2() {
        let fm = "generated: { by: 'did:nostr:not-hex', at: '2026-09-22T00:00:00Z' }\n";
        let v3 = run_k(fm, 3, None);
        let issue = v3
            .issues
            .iter()
            .find(|i| i.code == "INVALID_STAMP")
            .unwrap();
        assert_eq!(issue.severity, crate::check::validate::Severity::Error);
        let v2 = run_k(fm, 2, None);
        let issue = v2
            .issues
            .iter()
            .find(|i| i.code == "INVALID_STAMP")
            .unwrap();
        assert_eq!(issue.severity, crate::check::validate::Severity::Warning);
        assert!(v2.errors().is_empty(), "{:?}", codes(&v2));
    }

    #[test]
    fn v2_warns_about_a_dropped_stamp_only_once_per_stamp_and_never_for_a_good_one() {
        let good = run_k("generated: { by: 'process:vault/1.0', at: '2026-09-22T00:00:00Z' }\nverified: [{ by: 'human:npub1x', at: '2026-09-22' }]\n", 2, None);
        assert!(!has(&good, "INVALID_STAMP"), "{:?}", codes(&good));
        let bad = run_k("generated: { by: 'bot:x', at: '2026-09-22' }\nverified:\n  - { by: 'human:npub1x' }\n  - { by: 'human:npub1x', at: '2026-09-22' }\n", 2, None);
        assert_eq!(count(&bad, "INVALID_STAMP"), 2);
        let empty = run_k("verified:\n", 2, None);
        assert!(!has(&empty, "INVALID_STAMP"));
    }

    #[test]
    fn both_stamp_grammars_parse_under_either_version() {
        for version in [2, 3] {
            let r = run_k(
                &format!("generated: {{ by: {B4}, at: '2026-09-22T00:00:00Z' }}\n"),
                version,
                None,
            );
            assert!(
                !has(&r, "INVALID_STAMP") && !has(&r, "MISSING_GENERATED"),
                "v{version}: {:?}",
                codes(&r)
            );
        }
        // The vocabulary 2 spelling is read under v2 only (ruling 2): under v3
        // it is an error naming the migration.
        let legacy =
            format!("generated: {{ by: 'human:{D5_NPUB}', at: '2026-09-22T00:00:00Z' }}\n");
        assert!(!has(&run_k(&legacy, 2, None), "INVALID_STAMP"));
        let r = run_k(&legacy, 3, None);
        assert!(
            has(&r, "INVALID_STAMP") && !r.errors().is_empty(),
            "{:?}",
            codes(&r)
        );
    }

    #[test]
    fn v3_refuses_what_is_not_a_principal() {
        for by in [
            "human:jjohare",
            "human:npub1abc",
            "agent:enrichment-swarm/6",
            "process:vault-migrate/1.0",
        ] {
            let r = run_k(
                &format!("generated: {{ by: '{by}', at: '2026-09-22T00:00:00Z' }}\n"),
                3,
                None,
            );
            assert!(has(&r, "INVALID_STAMP"), "{by}: {:?}", codes(&r));
        }
        // A missing or blank `at`, a non-mapping stamp and a missing `by`.
        for fm in [
            format!("generated: {{ by: {B4} }}\n"),
            format!("generated: {{ by: {B4}, at: '' }}\n"),
            "generated: plain text\n".to_string(),
            "generated: { at: '2026-09-22T00:00:00Z' }\n".to_string(),
        ] {
            let r = run_k(&fm, 3, None);
            assert!(has(&r, "INVALID_STAMP"), "{fm}: {:?}", codes(&r));
        }
    }

    #[test]
    fn via_must_be_an_agent_or_a_process() {
        let ok = run_k(&format!("generated: {{ by: {B4}, via: 'process:vault-migrate/1.0', at: '2026-09-22T00:00:00Z' }}\n"), 3, None);
        assert!(!has(&ok, "INVALID_STAMP"));
        for via in [D5, "human:npub1x", "nonsense"] {
            let r = run_k(
                &format!("generated: {{ by: {B4}, via: '{via}', at: '2026-09-22T00:00:00Z' }}\n"),
                3,
                None,
            );
            assert!(has(&r, "INVALID_STAMP"), "{via}");
        }
    }

    #[test]
    fn verified_is_human_only_by_roster_kind() {
        let roster = roster();
        let human = run_k(
            &format!("verified: [{{ by: {B4}, at: '2026-11-04T00:00:00Z' }}]\n"),
            3,
            Some(&roster),
        );
        assert!(
            !has(&human, "VERIFIED_NOT_HUMAN") && !has(&human, "VERIFIED_PRINCIPAL_UNKNOWN"),
            "{:?}",
            codes(&human)
        );
        let org = run_k(
            &format!("verified: [{{ by: {ORG}, at: '2026-11-04T00:00:00Z' }}]\n"),
            3,
            Some(&roster),
        );
        assert!(has(&org, "VERIFIED_NOT_HUMAN"), "{:?}", codes(&org));
        let unknown = run_k(
            "verified: [{ by: 'did:nostr:0000000000000000000000000000000000000000000000000000000000000001', at: '2026-11-04T00:00:00Z' }]\n",
            3,
            Some(&roster),
        );
        assert!(has(&unknown, "VERIFIED_PRINCIPAL_UNKNOWN"));
        // No roster loaded: nothing to judge against, so nothing is claimed.
        let none = run_k(
            &format!("verified: [{{ by: {ORG}, at: '2026-11-04T00:00:00Z' }}]\n"),
            3,
            None,
        );
        assert!(!has(&none, "VERIFIED_NOT_HUMAN"));
    }

    #[test]
    fn a_key_listed_under_also_resolves_to_the_canonical_principal() {
        let roster = roster();
        let r = run_k(
            &format!("generated: {{ by: {D5}, at: '2026-09-22T00:00:00Z' }}\n"),
            3,
            Some(&roster),
        );
        assert!(has(&r, "NON_CANONICAL_PRINCIPAL"), "{:?}", codes(&r));
        let npub = run_k(
            &format!("generated: {{ by: 'human:{D5_NPUB}', at: '2026-09-22T00:00:00Z' }}\n"),
            3,
            Some(&roster),
        );
        // Ruling 2: the vocabulary 2 spelling is an error under v3, not a warning.
        assert!(has(&npub, "INVALID_STAMP") && !npub.errors().is_empty());
        let canonical = run_k(
            &format!("generated: {{ by: {B4}, at: '2026-09-22T00:00:00Z' }}\n"),
            3,
            Some(&roster),
        );
        assert!(!has(&canonical, "NON_CANONICAL_PRINCIPAL"));
        // The verified rule follows the key to the canonical principal's kind.
        let verified = run_k(
            &format!("verified: [{{ by: {D5}, at: '2026-11-04T00:00:00Z' }}]\n"),
            3,
            Some(&roster),
        );
        assert!(!has(&verified, "VERIFIED_NOT_HUMAN"));
    }

    #[test]
    fn contributors_follow_the_same_shape() {
        let ok = run_k(&format!("contributors:\n  - {{ by: {B4}, via: 'agent:x/1', at: '2026-11-23T14:02:00Z' }}\n  - {{ by: {B4}, at: '2026-11-23T15:00:00Z' }}\n"), 3, None);
        assert!(!has(&ok, "INVALID_STAMP"), "{:?}", codes(&ok));
        let bad = run_k(
            "contributors:\n  - { by: 'agent:x/1', at: '2026-11-23T14:02:00Z' }\n  - plain\n",
            3,
            None,
        );
        assert_eq!(count(&bad, "INVALID_STAMP"), 2);
    }

    /// A `verified` stamp by a roster principal of `kind: human` certifies,
    /// `did:nostr:` included, and the canonical key and its `also` keys alike;
    /// an organisation, an unknown key and no roster certify nothing.
    #[test]
    fn certification_counts_a_verified_did_stamp_by_a_human_principal() {
        use crate::input::okf::Certification;
        let roster = roster();
        let stamp = |by: &str| -> serde_norway::Value {
            serde_norway::from_str(&format!("[{{ by: {by}, at: '2026-11-04T00:00:00Z' }}]"))
                .unwrap()
        };
        let cert = |by: &str, r: &Roster| {
            Certification::from_verified_with(Some(&stamp(by)), &|a| r.is_human_actor(a))
        };
        assert!(cert(B4, &roster).whole_page);
        assert_eq!(cert(B4, &roster).state(), "certified");
        assert!(
            cert(D5, &roster).whole_page,
            "an `also` key resolves to the human"
        );
        assert!(
            !cert(ORG, &roster).whole_page,
            "an organisation never certifies"
        );
        assert!(
            !cert(B4, &Roster::default()).whole_page,
            "no roster: unknown key"
        );
        // Without a roster the legacy entry point is unchanged.
        assert!(!Certification::from_verified(Some(&stamp(B4))).whole_page);
        // Keys-scoped stamps stay scoped.
        let scoped: serde_norway::Value = serde_norway::from_str(&format!(
            "[{{ by: {B4}, at: '2026-11-04T00:00:00Z', keys: [is-a] }}]"
        ))
        .unwrap();
        let c = Certification::from_verified_with(Some(&scoped), &|a| roster.is_human_actor(a));
        assert_eq!(c.state(), "partial");
        assert_eq!(c.keys, ["is-a"]);
    }

    const SWARM: &str =
        "did:nostr:94773a699927da1ef879b90843890b1fe0e671ae898163f4337dcbe5f96992c0";

    /// B1: a process key filed under a human principal is an instrument, not a
    /// person. It does not certify, `verified` by it is an error, and the
    /// page-level checks agree with the stamp-level one.
    #[test]
    fn an_instrument_key_filed_under_a_human_is_not_human() {
        use crate::input::okf::Certification;
        let roster = Roster::from_yaml_str(&format!(
            "principals:\n  - did: {B4}\n    label: jjohare\n    kind: human\n    also: [{D5}]\ngenerators:\n  - did: {SWARM}\n    label: enrichment-swarm\n    kind: process\n    principal: {B4}\n"
        ));
        let r = run_k(
            &format!("verified: [{{ by: {SWARM}, at: '2026-11-04T00:00:00Z' }}]\n"),
            3,
            Some(&roster),
        );
        assert!(has(&r, "VERIFIED_NOT_HUMAN"), "{:?}", codes(&r));
        assert!(!r.errors().is_empty());
        let stamp: serde_norway::Value =
            serde_norway::from_str(&format!("[{{ by: {SWARM}, at: '2026-11-04T00:00:00Z' }}]"))
                .unwrap();
        let c = Certification::from_verified_with(Some(&stamp), &|a| roster.is_human_actor(a));
        assert!(!c.whole_page);
        assert_eq!(c.state(), "pending");
        // The operator's own keys still certify (Fungible Token's stamp).
        let own = run_k(
            &format!("verified: [{{ by: {B4}, at: '2026-11-04T00:00:00Z' }}]\n"),
            3,
            Some(&roster),
        );
        assert!(!has(&own, "VERIFIED_NOT_HUMAN"), "{:?}", codes(&own));
        let src = format!(
            "---\ntype: Class\nresource: urn:ngm:class:a\nstatus: stable\npublic: true\nverified:\n  - by: {SWARM}\n    at: '2026-11-04T00:00:00Z'\n---\n"
        );
        let page = crate::input::page::Page::parse(
            std::path::Path::new("/v/A.md"),
            "A.md",
            "A".to_owned(),
            &src,
        )
        .unwrap();
        assert_eq!(
            Certification::for_page_with(&page, Some(&roster)).state(),
            "pending"
        );
        let okf = crate::input::okf::OkfBlock::from_frontmatter(&page.frontmatter);
        assert!(!okf.is_human_verified_with(Some(&roster)));
    }

    /// S12 on a knowledge page: an empty `generated:` is as invalid there as on
    /// a working page.
    #[test]
    fn an_empty_generated_is_invalid_on_a_knowledge_page_too() {
        let r = run_k("generated:\n", 3, None);
        assert!(has(&r, "INVALID_STAMP"), "{:?}", codes(&r));
        assert!(!r.errors().is_empty());
        // Version 2 keeps ignoring it.
        assert!(!has(&run_k("generated:\n", 2, None), "INVALID_STAMP"));
    }

    /// The page API's certification (`for_page_with`, the parse crate's
    /// `assess_with`) agrees, and UNVERIFIED_STABLE follows the roster under v3.
    #[test]
    fn page_certification_and_human_verified_follow_the_roster() {
        let src = format!(
            "---\ntype: Class\nresource: urn:ngm:class:a\nstatus: stable\npublic: true\nverified:\n  - by: {B4}\n    at: '2026-11-04T00:00:00Z'\n---\n"
        );
        let page = crate::input::page::Page::parse(
            std::path::Path::new("/v/A.md"),
            "A.md",
            "A".to_owned(),
            &src,
        )
        .unwrap();
        let roster = roster();
        assert_eq!(
            crate::input::okf::Certification::for_page_with(&page, Some(&roster)).state(),
            "certified"
        );
        assert_eq!(
            crate::input::okf::Certification::for_page(&page).state(),
            "pending"
        );
        let okf = crate::input::okf::OkfBlock::from_frontmatter(&page.frontmatter);
        assert!(okf.is_human_verified_with(Some(&roster)));
        assert!(!okf.is_human_verified());
    }

    /// Item 1: under vocabulary 3 the retired `human:<npub or label>` spelling
    /// certifies nowhere (page certification, the stable-page warning, the
    /// roster's own test), with or without a roster; under vocabulary 2 the
    /// prefix rule is as released.
    #[test]
    fn the_old_human_spelling_certifies_only_under_version_2() {
        use crate::input::okf::{Certification, OkfBlock};
        use crate::input::vocabulary::Vocabulary;
        let v2 = Vocabulary::from_yaml_str("version: 2\n").unwrap();
        let v3 = Vocabulary::from_yaml_str("version: 3\n").unwrap();
        let roster = roster();
        for by in [
            "human:npub16kv2h3hdalphuyzj8tkycq8cm9n54ymer23558ds80afkc9e7frqwrawzj",
            "human:jjohare",
        ] {
            let src = format!(
                "---\ntype: Class\nresource: urn:ngm:class:a\nstatus: stable\npublic: true\nverified:\n  - by: {by}\n    at: '2026-11-04T00:00:00Z'\n---\n"
            );
            let page = crate::input::page::Page::parse(
                std::path::Path::new("/v/A.md"),
                "A.md",
                "A".to_owned(),
                &src,
            )
            .unwrap();
            let okf = OkfBlock::from_frontmatter(&page.frontmatter);
            for r in [None, Some(&roster)] {
                assert_eq!(
                    Certification::for_page_under(&page, &v3, r).state(),
                    "pending",
                    "{by}"
                );
                assert!(!okf.is_human_verified_under(&v3, r), "{by}");
                assert_eq!(
                    Certification::for_page_under(&page, &v2, r).state(),
                    "certified",
                    "{by}"
                );
                assert!(okf.is_human_verified_under(&v2, r), "{by}");
            }
            let stamp = &okf.verified[0].by;
            assert!(
                !roster.is_human_actor(stamp),
                "{by}: a roster never reads the prefix"
            );
            assert!(!roster.is_human_by(by));
        }
        // A did stamp by the operator certifies under 3 with the roster only.
        let src = format!(
            "---\ntype: Class\nresource: urn:ngm:class:a\nstatus: stable\npublic: true\nverified:\n  - by: {B4}\n    at: '2026-11-04T00:00:00Z'\n---\n"
        );
        let page = crate::input::page::Page::parse(
            std::path::Path::new("/v/A.md"),
            "A.md",
            "A".to_owned(),
            &src,
        )
        .unwrap();
        assert_eq!(
            Certification::for_page_under(&page, &v3, Some(&roster)).state(),
            "certified"
        );
        assert_eq!(
            Certification::for_page_under(&page, &v3, None).state(),
            "pending"
        );
    }
}
