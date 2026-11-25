//! Vocabulary version 3 validation (ADR-VG-010 to ADR-VG-013).
//!
//! Everything here runs only when the vault's vocabulary declares
//! `version: 3` or later ([`Vocabulary::is_v3`]); under version 2 the
//! validator never builds a [`Context`] and its output is unchanged.
//!
//! The rules are grouped by stage:
//!
//! | module | stage | subject |
//! |---|---|---|
//! | this one | S0 | the version switch, validation context, closed working enums |
//! | [`stamps`] | S1 | principal `by`, instrument `via`, human-only `verified` |
//! | [`temporal`] | S2 | `valid-from`/`valid-to`, supersession, statements |
//! | [`state`] | S4 | working-memory shape: visibility, `page_resource`, status sets, review state, `about`, recall |
//! | [`sources`] | S3 | typed sources, locators, pins, the memory kinds' own keys |
//!
//! Each rule reports through `Report::push` under a stable code; the codes
//! the four ADRs name are used verbatim.

use std::collections::HashMap;

pub mod sources;
pub mod stamps;
pub mod state;
pub mod temporal;

use crate::build::attribution::Roster;
use crate::check::validate::{Report, Severity};
use crate::input::frontmatter::Wikilink;
use crate::input::page::{Page, Vault, VaultKind};
use crate::input::vocabulary::Vocabulary;

/// What version-3 rules may resolve against beyond the vault being validated.
///
/// Every field is optional context. A rule that needs context it was not given
/// **skips** rather than reporting: validating a single vault in isolation
/// must not invent dangling references.
#[derive(Debug, Default, Clone)]
pub struct Siblings<'a> {
    /// The other loaded vaults (for example `knowledge/` while validating
    /// `working/`), used to resolve links and fingerprints across vaults.
    pub vaults: Vec<&'a Vault>,
    /// The attribution roster (`ontology/attribution.yaml`): which principal
    /// is human, which keys resolve to which. Absent means rules that need it
    /// say nothing.
    pub roster: Option<&'a Roster>,
}

/// A page reachable by reference, with the vault role it lives in.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    /// The page.
    pub page: &'a Page,
    /// The vault role it belongs to.
    pub kind: VaultKind,
}

/// Indexes built once per validated vault, shared by every version-3 rule.
#[derive(Debug)]
pub struct Context<'a> {
    /// The vault under validation.
    pub vault: &'a Vault,
    /// The other vaults supplied by the caller.
    pub siblings: &'a Siblings<'a>,
    /// Every page of the vault and its siblings, this vault first.
    targets: Vec<Target<'a>>,
    /// Lower-cased id, title and alias to `targets` positions.
    by_name: HashMap<String, Vec<usize>>,
    /// `resource` and `page_resource` values to `targets` positions.
    by_resource: HashMap<String, Vec<usize>>,
}

impl<'a> Context<'a> {
    /// The roster, when one was supplied.
    #[must_use]
    pub fn roster(&self) -> Option<&'a Roster> {
        self.siblings.roster
    }

    /// Index `vault` and its `siblings`.
    #[must_use]
    pub fn build(vault: &'a Vault, _vocab: &Vocabulary, siblings: &'a Siblings<'a>) -> Self {
        let targets: Vec<Target<'a>> = std::iter::once(vault)
            .chain(siblings.vaults.iter().copied())
            .flat_map(|v| v.pages.iter().map(|page| Target { page, kind: v.kind }))
            .collect();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_resource: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, t) in targets.iter().enumerate() {
            let names = std::iter::once(t.page.id.clone())
                .chain(std::iter::once(t.page.title()))
                .chain(t.page.frontmatter.strings("aliases"));
            for name in names {
                by_name.entry(name.to_lowercase()).or_default().push(i);
            }
            for key in ["resource", "page_resource"] {
                if let Some(value) = t.page.frontmatter.text(key) {
                    by_resource.entry(value).or_default().push(i);
                }
            }
        }
        Self {
            vault,
            siblings,
            targets,
            by_name,
            by_resource,
        }
    }

    /// `true` when pages of `kind` were loaded, so a reference into that
    /// vault can be judged dangling. A rule that cannot see the vault skips.
    #[must_use]
    pub fn sees(&self, kind: VaultKind) -> bool {
        self.vault.kind == kind || self.siblings.vaults.iter().any(|v| v.kind == kind)
    }

    /// Resolve a reference as written in frontmatter: a `[[wikilink]]` (page
    /// id, title or alias), or an IRI naming a page's `resource` or
    /// `page_resource`. `None` when nothing loaded matches.
    #[must_use]
    pub fn resolve(&self, reference: &str) -> Option<Target<'a>> {
        let index = if let Some(link) = Wikilink::parse(reference) {
            self.by_name
                .get(&link.target.to_lowercase())
                .and_then(|v| v.first())
        } else {
            self.by_resource
                .get(reference.trim())
                .and_then(|v| v.first())
        };
        index.map(|i| self.targets[*i])
    }
}

/// Per-page version-3 rules.
pub fn page_checks(report: &mut Report, page: &Page, vocab: &Vocabulary, ctx: &Context<'_>) {
    working_extension_enums(report, page, vocab, ctx);
    stamps::check_page(report, page, ctx);
    temporal::check_page(report, page, vocab, ctx);
    sources::check_page(report, page, vocab, ctx);
    state::check_page(report, page, vocab, ctx);
}

/// Rules that need the whole vault (cycles, targets): run after every page.
pub fn corpus_checks(report: &mut Report, vault: &Vault, vocab: &Vocabulary, ctx: &Context<'_>) {
    temporal::check_corpus(report, vault, vocab, ctx);
    state::check_corpus(report, vault);
}

/// `working_extensions` enums are closed under version 3 (ADR-VG-010 §6
/// `promotion-status`). Version 2 never validated them.
fn working_extension_enums(
    report: &mut Report,
    page: &Page,
    vocab: &Vocabulary,
    ctx: &Context<'_>,
) {
    if ctx.vault.kind != VaultKind::Working {
        return;
    }
    for (key, def) in &vocab.working_extensions {
        if def.r#enum.is_empty() {
            continue;
        }
        let Some(raw) = page.frontmatter.text(key) else {
            continue;
        };
        if !def.r#enum.contains(&raw) {
            report.push(
                &page.id,
                Severity::Error,
                "INVALID_WORKING_ENUM",
                format!("`{key}: {raw}` is not one of {:?}", def.r#enum),
            );
        }
    }
}

/// Shared helpers for the version-3 rule tests.
#[cfg(test)]
pub(crate) mod testing {
    use super::Siblings;
    use crate::check::validate::{validate_with, Report};
    use crate::input::page::{Page, Vault, VaultKind};
    use crate::input::vocabulary::Vocabulary;
    use crate::ir::model::Corpus;

    /// A vocabulary at `version` with the version-3 working types in map form,
    /// plus `extra` YAML appended.
    pub(crate) fn vocab(version: u32, extra: &str) -> Vocabulary {
        Vocabulary::from_yaml_str(&format!(
            r#"
version: {version}
namespace: "urn:ngm:class:"
types:
  Class: {{ owl: "owl:Class", required: [resource, status] }}
working_types:
  Note: {{}}
  Journal: {{}}
  Episode: {{ memory: true }}
  Brief: {{}}
  Proposal: {{}}
  Belief: {{ memory: true }}
  Procedure: {{ memory: true }}
relations:
  is-a:     {{ owl: "rdfs:subClassOf" }}
  requires: {{ owl: "vc:requires" }}
working_extensions:
  promotion-status: {{ type: text, enum: [candidate, candidate_deferred, candidate_rejected, elevated] }}
{extra}
"#
        ))
        .expect("the test vocabulary parses")
    }

    /// Build a vault of `(id, text)` pages.
    pub(crate) fn vault_of(pages: &[(&str, &str)], kind: VaultKind) -> Vault {
        Vault {
            root: std::path::PathBuf::new(),
            kind,
            journals: Vec::new(),
            skipped: Vec::new(),
            pages: pages
                .iter()
                .map(|(id, text)| {
                    Page::parse(format!("/v/{id}.md"), format!("{id}.md"), *id, text)
                        .expect("the test page parses")
                })
                .collect(),
        }
    }

    /// Validate `vault` under `vocab` with `siblings`.
    pub(crate) fn run_with(vault: &Vault, vocab: &Vocabulary, siblings: &Siblings<'_>) -> Report {
        let corpus = Corpus::build(vault, vocab);
        validate_with(
            &crate::profile::test_estate(),
            vault,
            &corpus,
            vocab,
            siblings,
        )
    }

    /// Validate a vault of `pages` of `kind` with no siblings.
    pub(crate) fn run(pages: &[(&str, &str)], kind: VaultKind, vocab: &Vocabulary) -> Report {
        run_with(&vault_of(pages, kind), vocab, &Siblings::default())
    }

    /// The issue codes of `report`, in report order.
    pub(crate) fn codes(report: &Report) -> Vec<String> {
        report.issues.iter().map(|i| i.code.clone()).collect()
    }

    /// `true` when `report` carries `code`.
    pub(crate) fn has(report: &Report, code: &str) -> bool {
        report.issues.iter().any(|i| i.code == code)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use crate::input::okf::Source;
    use crate::input::page::VaultKind;
    use crate::input::vocabulary::Vocabulary;

    const KNOWLEDGE: &str = "---\ntype: Class\npublic: true\nresource: urn:ngm:class:a\nstatus: draft\nvalid-from: 2026-01-01T00:00:00Z\n---\n";

    #[test]
    fn the_version_switch_reads_the_declared_version() {
        assert!(!vocab(2, "").is_v3());
        assert!(vocab(3, "").is_v3());
        assert!(vocab(4, "").is_v3());
        assert!(!Vocabulary::generic().is_v3());
    }

    #[test]
    fn a_v3_knowledge_key_is_unknown_under_v2_and_known_under_v3() {
        let under_v2 = run(&[("A", KNOWLEDGE)], VaultKind::Knowledge, &vocab(2, ""));
        assert!(has(&under_v2, "UNKNOWN_KEY"), "{:?}", codes(&under_v2));
        let under_v3 = run(&[("A", KNOWLEDGE)], VaultKind::Knowledge, &vocab(3, ""));
        assert!(!has(&under_v3, "UNKNOWN_KEY"), "{:?}", codes(&under_v3));
    }

    #[test]
    fn working_types_accept_the_list_and_the_map_form() {
        let list =
            Vocabulary::from_yaml_str("version: 3\nworking_types: [Note, Episode]\n").unwrap();
        assert!(list.working_types.contains("Episode") && list.working_type_defs.is_empty());
        let map = Vocabulary::from_yaml_str(
            "version: 3\nworking_types:\n  Note: {}\n  Belief: { required: [about], status: [draft, approved] }\n",
        )
        .unwrap();
        assert_eq!(map.working_types.len(), 2);
        assert_eq!(map.working_type_defs["Belief"].required, ["about"]);
        assert_eq!(map.working_type("Belief").status, ["draft", "approved"]);
    }

    #[test]
    fn the_built_in_memory_kinds_are_memory_even_in_list_form() {
        let list =
            Vocabulary::from_yaml_str("version: 3\nworking_types: [Note, Belief]\n").unwrap();
        assert!(list.working_type("Belief").memory);
        assert!(!list.working_type("Note").memory);
    }

    #[test]
    fn a_bad_working_types_shape_is_refused() {
        assert!(Vocabulary::from_yaml_str("version: 3\nworking_types: 4\n").is_err());
        assert!(Vocabulary::from_yaml_str("version: 3\nworking_types:\n  Note: 3\n").is_err());
    }

    const BOGUS_PROMOTION: &str = "---\ntype: Note\npromotion-status: bogus\n---\n";
    const GOOD_PROMOTION: &str = "---\ntype: Note\npromotion-status: candidate_deferred\n---\n";

    #[test]
    fn promotion_status_is_a_closed_enum_under_v3_only() {
        let v2 = run(&[("N", BOGUS_PROMOTION)], VaultKind::Working, &vocab(2, ""));
        assert!(!has(&v2, "INVALID_WORKING_ENUM"), "{:?}", codes(&v2));
        let bad = run(&[("N", BOGUS_PROMOTION)], VaultKind::Working, &vocab(3, ""));
        assert!(has(&bad, "INVALID_WORKING_ENUM"), "{:?}", codes(&bad));
        let good = run(&[("N", GOOD_PROMOTION)], VaultKind::Working, &vocab(3, ""));
        assert!(!has(&good, "INVALID_WORKING_ENUM"), "{:?}", codes(&good));
    }

    #[test]
    fn a_version_2_vocabulary_ignores_every_v3_rule() {
        let v2 = vocab(2, "");
        let plain = run(&[("N", BOGUS_PROMOTION)], VaultKind::Working, &v2);
        let again = crate::check::validate::validate(
            &crate::profile::test_estate(),
            &vault_of(&[("N", BOGUS_PROMOTION)], VaultKind::Working),
            &crate::ir::model::Corpus::build(
                &vault_of(&[("N", BOGUS_PROMOTION)], VaultKind::Working),
                &v2,
            ),
            &v2,
        );
        assert_eq!(codes(&plain), codes(&again));
    }

    #[test]
    fn a_source_round_trips_every_field_it_does_not_model() {
        let yaml: serde_norway::Value = serde_norway::from_str(
            "{ id: ep, resource: '[[Ledger]]', rel: evidence, at: '2026-11-01', locator: 'fp:ab12', note: kept, extra: { n: 1 } }",
        )
        .unwrap();
        let source = Source::from_yaml(&yaml).unwrap();
        assert_eq!(source.rel.as_deref(), Some("evidence"));
        assert_eq!(source.locator.as_deref(), Some("fp:ab12"));
        assert_eq!(source.extensions.len(), 2);
        let back = Source::from_yaml(&source.to_yaml()).unwrap();
        assert_eq!(back, source);
        let rendered = source.to_yaml();
        assert_eq!(rendered.get("note").and_then(|v| v.as_str()), Some("kept"));
        assert!(rendered.get("extra").is_some());
    }

    #[test]
    fn a_plain_source_renders_exactly_as_before() {
        let source = Source::new("origin", "[[Notes]]");
        let rendered = serde_norway::to_string(&source.to_yaml()).unwrap();
        assert_eq!(rendered, "id: origin\nresource: '[[Notes]]'\n");
    }

    #[test]
    fn a_source_missing_its_id_is_still_not_a_source_for_the_typed_reader() {
        let yaml: serde_norway::Value =
            serde_norway::from_str("{ rel: origin, resource: 'https://x.example/v' }").unwrap();
        assert!(Source::from_yaml(&yaml).is_none());
    }
}
