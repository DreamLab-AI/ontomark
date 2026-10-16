//! [`PatchProposal`] — contract C4, the payload of a forum 31402
//! `ActionRequest`.
//!
//! The invariant the whole governance loop rests on: **a proposal with a
//! non-empty `blockers` list is never posted.** `vault propose` runs Whelk and
//! the conflict detector first, collects everything they find, and emits the
//! proposal with its blockers so the refusal is auditable — but it does not
//! publish it. `ontomark apply` is the receiving end: it re-computes the
//! digest over the proposal's fields and refuses one that does not match.
//!
//! The digest functions here are **bit-identical** to VisionClaw
//! `vault-core/src/proposal.rs`: a proposal already signed and on the relay
//! must verify under `ontomark apply` exactly as it did under `vault apply`.
//! The pinned test vector in this module's tests is the one `vault-core`
//! carries.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::input::promotion::Blocker;
use crate::input::slug::slugify;

/// The namespace a **grouped** proposal's own subject IRI is minted in:
/// `urn:ngm:proposal:<slug of its title>`. A grouped proposal is one decision
/// over many pages, so its subject is the decision, not any one page.
pub const PROPOSAL_NAMESPACE: &str = "urn:ngm:proposal:";

/// How far-reaching a proposal is. Schema-level changes are floored at risk
/// tier High by the panel operator (decision Q7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// A change to one page's content.
    Content,
    /// A change to the vocabulary or to the taxonomy's shape.
    Schema,
    /// A withdrawal.
    Demotion,
}

impl Level {
    /// The tag value used on the 31402 `level` tag.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Schema => "schema",
            Self::Demotion => "demotion",
        }
    }

    /// `true` when the panel must floor this proposal at risk tier High.
    #[must_use]
    pub fn floors_tier_high(self) -> bool {
        matches!(self, Self::Schema)
    }
}

impl std::str::FromStr for Level {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "content" => Ok(Self::Content),
            "schema" => Ok(Self::Schema),
            "demotion" => Ok(Self::Demotion),
            other => Err(format!(
                "unknown level {other:?}; expected content, schema or demotion"
            )),
        }
    }
}

/// Whether a proposal changes pages that exist or brings a new one into being.
///
/// An **elevation** — a `working/` note promoted to a new knowledge Class — is
/// a creation: its diff is against `/dev/null`, and the apply side writes a new
/// file rather than editing one (`ontomark create`, not `ontomark edit`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalKind {
    /// Every page the proposal names already exists.
    #[default]
    Amend,
    /// At least one page the proposal names does not exist yet; the diff's
    /// `--- /dev/null` headers say which.
    Create,
}

impl ProposalKind {
    /// The JSON and tag value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Amend => "amend",
            Self::Create => "create",
        }
    }
}

/// The C4 JSON document.
///
/// Field order is the contract's order, and the struct is serialised with
/// `preserve_order` maps nowhere in sight, so the JSON is stable across runs —
/// which matters because [`PatchProposal::digest`] hashes it.
///
/// ```
/// use ontomark_compile::write::proposal::{Level, PatchProposal, ProposalKind};
/// let p = PatchProposal::new(
///     Level::Content, "urn:ngm:class:a", "A", "why", "--- a/A\n+++ b/A\n",
///     vec![], "process:ontomark/0.2", "visionGraph@abc", "2026-10-30T00:00:00Z",
/// );
/// assert_eq!(p.kind, ProposalKind::Amend);
/// assert!(p.is_postable());
/// assert_eq!(p.digest, PatchProposal::compute_digest(
///     Level::Content, "urn:ngm:class:a", "A", "why", "--- a/A\n+++ b/A\n"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchProposal {
    /// Content, schema or demotion.
    pub level: Level,
    /// Amend existing pages, or create a new one. Absent in a proposal
    /// serialised before creation existed, which was always an amendment.
    #[serde(default)]
    pub kind: ProposalKind,
    /// The subject's stable IRI. For a grouped proposal, the proposal's own
    /// IRI in [`PROPOSAL_NAMESPACE`].
    pub iri: String,
    /// The subject's page id. A grouped proposal has no single page, so this
    /// repeats its [`PatchProposal::iri`]; [`PatchProposal::pages`] lists the
    /// pages it changes.
    pub page: String,
    /// What the proposer believes and why. Free text, shown to the human.
    pub hypothesis: String,
    /// A unified diff of the page's frontmatter.
    pub diff: String,
    /// `sha256:<hex>` over level, iri, page, hypothesis and diff.
    pub digest: String,
    /// Automatic refusals. Non-empty means the proposal is not posted.
    ///
    /// Blockers are **deltas**: a conflict, validation error or
    /// unsatisfiable class present after the change and absent before it.
    pub blockers: Vec<Blocker>,
    /// Findings on the pages this proposal touches that already existed
    /// before it. Informational only — they never stop a post, because a
    /// proposal cannot be held responsible for a defect it did not introduce.
    #[serde(default)]
    pub preexisting: Vec<Blocker>,
    /// The proposing actor, e.g. `process:vault/1.0`.
    pub proposer: String,
    /// The build generation the proposal was computed against.
    pub generation: String,
    /// ISO-8601 instant after which the proposal reverts to draft.
    pub stale_after: String,
    /// Every page this proposal changes, sorted, including the subject.
    ///
    /// A single-page proposal holds exactly `[page]`. A **grouped** proposal
    /// holds all of them, and `diff` is then a multi-file unified diff.
    ///
    /// Grouping exists because the two largest decisions in the corpus are not
    /// expressible one page at a time: folding `domain: ai` into
    /// `artificial-intelligence` touches 513 pages and the journal base-copy
    /// choice touches 125. Filing those as 513 and 125 separate content
    /// proposals would put a *schema*-shaped decision through the content tier
    /// 638 times, and a human would be asked to sign the same judgement over and
    /// over with no way to accept or reject it as one thing.
    #[serde(default)]
    pub pages: Vec<String>,
}

/// How long a proposal stays live before it expires (decision Q7).
pub const STALE_AFTER_DAYS: i64 = 14;

impl PatchProposal {
    /// Assemble a proposal and compute its digest.
    ///
    /// `stale_after` is supplied by the caller rather than read from the clock
    /// so the whole type stays deterministic and testable.
    ///
    /// The nine parameters are the nine fields of contract C4 minus the two
    /// this constructor derives; collapsing them into a builder would hide the
    /// contract rather than express it.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        level: Level,
        iri: impl Into<String>,
        page: impl Into<String>,
        hypothesis: impl Into<String>,
        diff: impl Into<String>,
        blockers: Vec<Blocker>,
        proposer: impl Into<String>,
        generation: impl Into<String>,
        stale_after: impl Into<String>,
    ) -> Self {
        let (iri, page) = (iri.into(), page.into());
        let (hypothesis, diff) = (hypothesis.into(), diff.into());
        let digest = Self::compute_digest(level, &iri, &page, &hypothesis, &diff);
        Self {
            level,
            kind: ProposalKind::Amend,
            iri,
            pages: vec![page.clone()],
            page,
            hypothesis,
            diff,
            digest,
            blockers,
            preexisting: Vec::new(),
            proposer: proposer.into(),
            generation: generation.into(),
            stale_after: stale_after.into(),
        }
    }

    /// Assemble a **grouped** proposal: one decision over `pages`, whose
    /// subject is the decision itself — `iri` and `page` are both
    /// `urn:ngm:proposal:<slug of title>`, and `pages` is the sorted,
    /// deduplicated set of pages changed.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn grouped(
        level: Level,
        title: &str,
        hypothesis: impl Into<String>,
        diff: impl Into<String>,
        blockers: Vec<Blocker>,
        proposer: impl Into<String>,
        generation: impl Into<String>,
        stale_after: impl Into<String>,
        pages: impl IntoIterator<Item = String>,
    ) -> Self {
        let iri = format!("{PROPOSAL_NAMESPACE}{}", slugify(title));
        let mut pages: Vec<String> = pages.into_iter().collect();
        pages.sort();
        pages.dedup();
        let (hypothesis, diff) = (hypothesis.into(), diff.into());
        let digest = Self::compute_digest_grouped(level, &iri, &iri, &hypothesis, &diff, &pages);
        Self {
            level,
            kind: ProposalKind::Amend,
            page: iri.clone(),
            iri,
            hypothesis,
            diff,
            digest,
            blockers,
            preexisting: Vec::new(),
            proposer: proposer.into(),
            generation: generation.into(),
            stale_after: stale_after.into(),
            pages,
        }
    }

    /// Attach the informational pre-existing findings. They are outside the
    /// digest and never affect [`PatchProposal::is_postable`].
    #[must_use]
    pub fn with_preexisting(mut self, preexisting: Vec<Blocker>) -> Self {
        self.preexisting = preexisting;
        self
    }

    /// Mark the proposal as a creation (or back to an amendment), recomputing
    /// the digest.
    ///
    /// The kind is folded into the digest only for a creation, so every
    /// amendment's digest — its 31402 `d` tag and case id — is byte-for-byte
    /// what it was before creation existed.
    #[must_use]
    pub fn with_kind(mut self, kind: ProposalKind) -> Self {
        self.kind = kind;
        self.digest = self.expected_digest();
        self
    }

    /// The digest this proposal's own fields hash to — what `vault propose`
    /// computed when it built it. Equal to [`PatchProposal::digest`] unless a
    /// field was changed afterwards.
    #[must_use]
    pub fn expected_digest(&self) -> String {
        Self::compute_digest_of_kind(
            self.level,
            self.kind,
            &self.iri,
            &self.page,
            &self.hypothesis,
            &self.diff,
            &self.pages,
        )
    }

    /// Declare the full page set of a **grouped** proposal, recomputing the
    /// digest.
    ///
    /// The subject is kept at the head of [`PatchProposal::page`] — it is what
    /// names the case — and `pages` becomes the whole set, sorted and
    /// deduplicated.
    #[must_use]
    pub fn with_pages(mut self, pages: impl IntoIterator<Item = String>) -> Self {
        let mut pages: Vec<String> = pages.into_iter().collect();
        pages.sort();
        pages.dedup();
        if !pages.contains(&self.page) {
            pages.push(self.page.clone());
            pages.sort();
        }
        self.pages = pages;
        self.digest = self.expected_digest();
        self
    }

    /// `true` when the proposal changes more than its subject.
    #[must_use]
    pub fn is_grouped(&self) -> bool {
        self.pages.len() > 1
    }

    /// `sha256:<hex>` over the semantic fields, newline-separated.
    ///
    /// `blockers`, `proposer`, `generation` and `stale_after` are deliberately
    /// *outside* the digest: the same proposed change must hash identically
    /// whoever proposes it and whenever, so a re-proposal after a blocker is
    /// cleared lands on the same case id.
    #[must_use]
    pub fn compute_digest(
        level: Level,
        iri: &str,
        page: &str,
        hypothesis: &str,
        diff: &str,
    ) -> String {
        Self::compute_digest_grouped(level, iri, page, hypothesis, diff, &[])
    }

    /// The digest, including a **grouped** proposal's page set.
    ///
    /// The set is folded in only when it names more than the subject, so a
    /// single-page proposal's digest — and therefore its 31402 `d` tag and its
    /// case id — is exactly what it was before grouping existed. A grouped
    /// proposal must hash differently from a single-page one over the same
    /// subject, or two genuinely different decisions would share a case.
    #[must_use]
    pub fn compute_digest_grouped(
        level: Level,
        iri: &str,
        page: &str,
        hypothesis: &str,
        diff: &str,
        pages: &[String],
    ) -> String {
        Self::compute_digest_of_kind(
            level,
            ProposalKind::Amend,
            iri,
            page,
            hypothesis,
            diff,
            pages,
        )
    }

    /// The digest, including the proposal's [`ProposalKind`].
    ///
    /// An amendment hashes exactly as [`PatchProposal::compute_digest_grouped`]
    /// always has; a creation appends `kind:create`, so a creation never shares
    /// a case with an amendment over the same subject.
    #[must_use]
    pub fn compute_digest_of_kind(
        level: Level,
        kind: ProposalKind,
        iri: &str,
        page: &str,
        hypothesis: &str,
        diff: &str,
        pages: &[String],
    ) -> String {
        let mut h = Sha256::new();
        for part in [level.as_str(), iri, page, hypothesis, diff] {
            h.update(part.as_bytes());
            h.update(b"\n");
        }
        if pages.len() > 1 {
            for p in pages {
                h.update(p.as_bytes());
                h.update(b"\n");
            }
        }
        if kind == ProposalKind::Create {
            h.update(b"kind:create\n");
        }
        format!("sha256:{}", hex::encode(h.finalize()))
    }

    /// `true` when nothing blocks the proposal and it may be posted.
    #[must_use]
    pub fn is_postable(&self) -> bool {
        self.blockers.is_empty()
    }

    /// The bare hex digest, for use as the 31402 `d` tag.
    #[must_use]
    pub fn digest_hex(&self) -> &str {
        self.digest.strip_prefix("sha256:").unwrap_or(&self.digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal(blockers: Vec<Blocker>) -> PatchProposal {
        PatchProposal::new(
            Level::Content,
            "urn:ngm:class:knowledge-graph",
            "Knowledge Graph",
            "quality is understated",
            "-quality: 0.35\n+quality: 0.55\n",
            blockers,
            "process:vault/1.0",
            "visionGraph@abc1234",
            "2026-10-06T00:00:00Z",
        )
    }

    #[test]
    fn a_clean_proposal_is_postable() {
        assert!(proposal(vec![]).is_postable());
    }

    #[test]
    fn any_blocker_stops_the_post() {
        let p = proposal(vec![Blocker::new("WHELK_INCONSISTENT", "owl:Nothing")]);
        assert!(!p.is_postable());
    }

    #[test]
    fn the_digest_ignores_proposer_and_expiry() {
        let a = proposal(vec![]);
        let mut b = proposal(vec![Blocker::new("X", "y")]);
        b.proposer = "agent:other/9".into();
        b.stale_after = "2099-01-01T00:00:00Z".into();
        assert_eq!(a.digest, b.digest);
    }

    #[test]
    fn the_digest_tracks_the_diff() {
        let a = proposal(vec![]);
        let b = PatchProposal::new(
            Level::Content,
            "urn:ngm:class:knowledge-graph",
            "Knowledge Graph",
            "quality is understated",
            "-quality: 0.35\n+quality: 0.65\n",
            vec![],
            "process:vault/1.0",
            "visionGraph@abc1234",
            "2026-10-06T00:00:00Z",
        );
        assert_ne!(a.digest, b.digest);
    }

    #[test]
    fn digest_hex_strips_the_algorithm_prefix() {
        let p = proposal(vec![]);
        assert_eq!(p.digest_hex().len(), 64);
        assert!(!p.digest_hex().contains(':'));
    }

    #[test]
    fn a_grouped_proposal_is_its_own_subject() {
        let pages = vec!["B".to_owned(), "A".to_owned(), "B".to_owned()];
        let p = PatchProposal::grouped(
            Level::Schema,
            "Fold domain ai into artificial-intelligence",
            "one root spelled two ways",
            "diff",
            vec![],
            "process:vault/1.0",
            "visionGraph@abc",
            "2026-10-06T00:00:00Z",
            pages,
        );
        assert_eq!(
            p.iri,
            "urn:ngm:proposal:fold-domain-ai-into-artificial-intelligence"
        );
        assert_eq!(p.page, p.iri);
        assert_eq!(p.pages, vec!["A", "B"]);
        assert!(p.is_grouped());
        assert_eq!(p.digest, p.expected_digest());
    }

    #[test]
    fn with_pages_keeps_the_subject_and_rekeys_the_digest() {
        let p = proposal(vec![]);
        let g = p.clone().with_pages(["Other".to_owned()]);
        assert_eq!(g.pages, vec!["Knowledge Graph", "Other"]);
        assert_ne!(g.digest, p.digest);
        assert_eq!(g.digest, g.expected_digest());
    }

    #[test]
    fn preexisting_findings_never_block() {
        let p = proposal(vec![]).with_preexisting(vec![Blocker::new("SUBCLASS_CYCLE", "old")]);
        assert!(p.is_postable());
        assert_eq!(p.digest, proposal(vec![]).digest, "outside the digest");
    }

    #[test]
    fn an_amendment_digest_is_the_pre_kind_digest() {
        // Pinned: the digest of `proposal(vec![])` before `kind` existed, as
        // vault-core pins it. A change here re-keys every amendment case
        // already on the relay.
        let p = proposal(vec![]);
        assert_eq!(p.kind, ProposalKind::Amend);
        assert_eq!(p.clone().with_kind(ProposalKind::Amend).digest, p.digest);
        let mut h = Sha256::new();
        for part in [
            "content",
            "urn:ngm:class:knowledge-graph",
            "Knowledge Graph",
            "quality is understated",
            "-quality: 0.35\n+quality: 0.55\n",
        ] {
            h.update(part.as_bytes());
            h.update(b"\n");
        }
        assert_eq!(p.digest, format!("sha256:{}", hex::encode(h.finalize())));
        // The literal value (`printf '<fields>\n' | sha256sum`), so a hashing
        // change cannot hide behind a shared helper.
        assert_eq!(
            p.digest,
            "sha256:4f404501541a144a6a82250b78015319fda7a01b6a0236156e45a033881c7ce9"
        );
    }

    #[test]
    fn a_creation_hashes_apart_from_an_amendment() {
        let amend = proposal(vec![]);
        let create = amend.clone().with_kind(ProposalKind::Create);
        assert_ne!(amend.digest, create.digest);
        let json = serde_json::to_value(&create).unwrap();
        assert_eq!(json["kind"], "create");
        // A proposal serialised before `kind` existed reads back as an amendment.
        let mut old = serde_json::to_value(&amend).unwrap();
        old.as_object_mut().unwrap().remove("kind");
        let back: PatchProposal = serde_json::from_value(old).unwrap();
        assert_eq!(back.kind, ProposalKind::Amend);
    }

    #[test]
    fn levels_parse_and_schema_floors_the_tier() {
        assert_eq!("Schema".parse::<Level>(), Ok(Level::Schema));
        assert!("other".parse::<Level>().is_err());
        assert!(Level::Schema.floors_tier_high());
        assert!(!Level::Content.floors_tier_high());
    }

    #[test]
    fn the_c4_json_shape_is_stable() {
        let json = serde_json::to_value(proposal(vec![])).unwrap();
        for key in [
            "level",
            "kind",
            "iri",
            "page",
            "hypothesis",
            "diff",
            "digest",
            "blockers",
            "proposer",
            "generation",
            "stale_after",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["level"], "content");
        assert_eq!(json["kind"], "amend");
    }
}
