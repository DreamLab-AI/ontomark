//! The public build boundary — a port of `pipeline/public_projection.py`.
//!
//! This is the security-relevant half of the build. Three things happen here
//! and all three must keep happening:
//!
//! 1. **Input census** ([`inspect_inputs`]). Every markdown file under
//!    `pages/` is accounted for before the parser gets a chance to silently
//!    drop it. A `public` value that is not a real boolean is refusal, not
//!    coercion — "true" the string never becomes publication permission.
//! 2. **Identity collision check.** If one identity (page id, slug or IRI) is
//!    claimed by both a public and a private page, the build stops. Otherwise a
//!    rename could leak a private page through a public IRI.
//! 3. **Reference redaction** ([`project`]). Filtering precedes the closure, so
//!    a private intermediate class never enters inferred ancestry; and private
//!    *names* occurring in prose are replaced with `[private]`, not just typed
//!    edges dropped.
//!
//! Pages under a dot-directory or an unpublished namespace are force-marked
//! private and carried into the marker set, so an incoming link cannot make a
//! held page public.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use crate::input::frontmatter;
use crate::input::page::is_unpublished;
use crate::input::page::Vault;
use crate::input::vocabulary::Vocabulary;
use indexmap::IndexMap;
use regex::{Regex, RegexBuilder};

use crate::input::slug::slugify;
use crate::ir::model::{ClassRecord, Conjunct, Corpus, EntityType, Ref};
use crate::profile::Profile;

/// An input could not safely cross the public build boundary.
#[derive(Debug, thiserror::Error)]
pub enum PublicationBlocked {
    /// The census found inputs it cannot classify.
    #[error("public build refused input census: {0}")]
    Census(String),
    /// One identity is claimed by both a public and a private page.
    #[error("public/private identity collision on {0}; resolve in the authoring source")]
    Collision(String),
    /// The private-marker redactor could not be compiled, so prose redaction
    /// cannot be guaranteed. The build refuses rather than publish unredacted
    /// text (S1.1).
    #[error("public build refused: the private-marker redactor did not compile ({0})")]
    Redactor(String),
}

/// The `census.json` document.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Census {
    /// Schema version of the census policy.
    pub policy_version: u32,
    /// Pages with `public: true`.
    pub public_pages: usize,
    /// Pages with `public: false` or no flag.
    pub non_public_pages: usize,
    /// Always zero: a rejected input raises instead of being counted.
    pub rejected: usize,
    /// Files skipped by directory rule or missing frontmatter.
    pub excluded: usize,
    /// Files that produced a page.
    pub parsed: usize,
    /// Every raw counter, so the arithmetic is auditable.
    pub counts: BTreeMap<String, usize>,
    /// `true` when every input file is accounted for exactly once.
    pub balanced: bool,
}

/// Walk `pages_dir` and account for every markdown file.
///
/// # Errors
/// [`PublicationBlocked::Census`] when any file has an invalid or conflicting
/// publication flag, an unclosed frontmatter fence, or cannot be read.
pub fn inspect_inputs(pages_dir: impl AsRef<Path>) -> Result<Census, PublicationBlocked> {
    let pages_dir = pages_dir.as_ref();
    let walk = crate::input::page::walk_pages(pages_dir).map_err(|e| {
        PublicationBlocked::Census(format!("cannot walk {}: {e}", pages_dir.display()))
    })?;
    inspect_walk(pages_dir, &walk)
}

/// [`inspect_inputs`] over a walk the caller already made, so a build walks
/// `pages/` once for the census and the load (OM-00 step 1).
///
/// The census and the vault load now share one listing and one skip rule: a
/// hidden-directory skip counts as `excluded_directory`, a non-regular `.md`
/// entry as `excluded_not_regular_file`, and both are visible in the counts.
///
/// # Errors
/// As [`inspect_inputs`].
pub fn inspect_walk(
    pages_dir: &Path,
    walk: &crate::input::page::PageWalk,
) -> Result<Census, PublicationBlocked> {
    use crate::input::page::SkipReason;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut bump = |k: &str| *counts.entry(k.to_owned()).or_insert(0) += 1;

    for skip in &walk.skipped {
        bump("input_files");
        bump(match skip.reason {
            SkipReason::HiddenDirectory => "excluded_directory",
            SkipReason::NotARegularFile => "excluded_not_regular_file",
        });
    }
    for path in &walk.files {
        bump("input_files");
        let rel = path.strip_prefix(pages_dir).unwrap_or(path);
        if is_unpublished(rel) {
            bump("excluded_directory");
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            bump("invalid_input");
            continue;
        };
        let split = frontmatter::split(&text);
        if split.unterminated {
            // An opening fence with no close is a malformed page, not a
            // private one (S3.3).
            bump("invalid_input");
            continue;
        }
        let Some(yaml) = split.yaml else {
            bump("excluded_no_frontmatter");
            continue;
        };
        let Ok(fm) = frontmatter::Frontmatter::parse(yaml) else {
            bump("invalid_input");
            continue;
        };
        // Never coerce a string or a number into publication permission: only a
        // real YAML boolean counts, and anything else refuses the build.
        match fm.get("public") {
            Some(serde_yaml::Value::Bool(true)) => bump("included_public"),
            None | Some(serde_yaml::Value::Bool(false)) => bump("included_private"),
            Some(_) => bump("invalid_publication_flag"),
        }
    }

    let invalid: usize = counts
        .iter()
        .filter(|(k, _)| k.starts_with("invalid_") || k.starts_with("conflicting_"))
        .map(|(_, v)| *v)
        .sum();
    if invalid > 0 {
        return Err(PublicationBlocked::Census(
            serde_json::to_string(&counts).unwrap_or_default(),
        ));
    }

    let get = |k: &str| counts.get(k).copied().unwrap_or(0);
    let input_files = get("input_files");
    let accounted: usize = counts
        .iter()
        .filter(|(k, _)| k.as_str() != "input_files")
        .map(|(_, v)| *v)
        .sum();

    Ok(Census {
        policy_version: 1,
        public_pages: get("included_public"),
        non_public_pages: get("included_private"),
        // Always 0: a page the census cannot classify refuses the whole build
        // above, so nothing is ever "rejected" in a census that is returned.
        // Kept because `api/census.json` is a published shape.
        rejected: 0,
        excluded: get("excluded_directory")
            + get("excluded_no_frontmatter")
            + get("excluded_not_regular_file"),
        parsed: get("included_public") + get("included_private"),
        balanced: input_files == accounted,
        counts,
    })
}

/// Identities belonging to **held** pages, so an incoming link cannot publish
/// one.
///
/// A held page is a page that exists, is loaded by the vault, is reachable by
/// `validate`, `find` and `retrieve`, and is deliberately never published: an
/// [`crate::input::page::UNPUBLISHED_DIRS`] folder. Its identities are collected
/// here so a link from a public page cannot drag it into the bundle.
///
/// A **dot-directory is not a held page** — `.deleted/` and `.trash/` are
/// tombstones, `.obsidian/` is editor state, and none of them is part of the
/// corpus at all (the vault walk reports them as skips). Collecting a
/// tombstone's identity blocked the live page that replaced it: the
/// `.deleted/Recurrent-Neural-Network.md` tombstone and the public
/// `Recurrent Neural Network.md` slugify to the same thing, and the build
/// refused the whole bundle over a page that had been deleted on purpose.
/// Deletion is not a publication hold.
///
/// # Errors
/// `std::io::Error` when the tree cannot be walked. (A page that fails to
/// parse is skipped: the vault load has already refused it by then.)
pub fn excluded_identities(pages_dir: impl AsRef<Path>) -> Result<HashSet<String>, std::io::Error> {
    let pages_dir = pages_dir.as_ref();
    let walk = crate::input::page::walk_pages(pages_dir)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut out = HashSet::new();
    for path in &walk.files {
        let rel = path.strip_prefix(pages_dir).unwrap_or(path);
        if !is_unpublished(rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let id = rel.with_extension("").to_string_lossy().replace('\\', "/");
        let Ok(page) = crate::input::page::Page::parse(path, rel, &id, &text) else {
            continue;
        };
        held_identities(&page, &id, &mut out);
    }
    Ok(out)
}

/// [`excluded_identities`] from a vault that is already loaded: the held pages
/// are in [`Vault::pages`] (the walk does not apply the publication rule), so
/// no second walk or parse is needed.
#[must_use]
pub fn excluded_identities_of(vault: &Vault) -> HashSet<String> {
    let mut out = HashSet::new();
    for page in &vault.pages {
        let rel = page
            .rel_path
            .strip_prefix("pages")
            .unwrap_or(&page.rel_path);
        if is_unpublished(rel) {
            held_identities(page, &page.id, &mut out);
        }
    }
    out
}

fn held_identities(page: &crate::input::page::Page, id: &str, out: &mut HashSet<String>) {
    out.insert(id.to_owned());
    out.insert(page.slug());
    if let Some(r) = page.frontmatter.text("resource") {
        out.insert(r);
    }
    out.insert(page.title());
}

/// A page's identity values: its id, slug, page IRI and class IRI.
fn identity(record: &ClassRecord) -> Vec<String> {
    [
        record.page_id.clone(),
        record.slug.clone(),
        record.page_iri.clone(),
        record.iri.clone(),
    ]
    .into_iter()
    .filter(|v| !v.is_empty())
    .collect()
}

/// The compiled-program size the private-marker alternation may reach
/// (`regex`'s own default). Past it the redactor **refuses** rather than runs
/// without a pattern: see [`PublicationBlocked::Redactor`].
pub const REDACTOR_SIZE_LIMIT: usize = 10 * (1 << 20);

/// The redactor: a compiled alternation over every private marker plus the
/// word-boundary rule Python expressed with look-around.
///
/// `pattern` is `None` only when there is nothing to redact. A compile failure
/// is an error, never an empty pattern (S1.1).
#[derive(Debug)]
struct Redactor {
    pattern: Option<Regex>,
    markers: HashSet<String>,
}

impl Redactor {
    fn new(markers: HashSet<String>) -> Result<Self, regex::Error> {
        Self::with_size_limit(markers, REDACTOR_SIZE_LIMIT)
    }

    fn with_size_limit(markers: HashSet<String>, size_limit: usize) -> Result<Self, regex::Error> {
        let mut sorted: Vec<&String> = markers.iter().filter(|m| !m.is_empty()).collect();
        // Longest first, then lexicographic, so the alternation is
        // deterministic and never matches a prefix of a longer marker.
        sorted.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        let pattern = if sorted.is_empty() {
            None
        } else {
            let alt: Vec<String> = sorted.iter().map(|m| regex::escape(m)).collect();
            Some(
                RegexBuilder::new(&alt.join("|"))
                    .size_limit(size_limit)
                    .build()?,
            )
        };
        Ok(Self { pattern, markers })
    }

    /// `\w` or `-` — the characters Python's look-around forbade on either side.
    fn is_boundary_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-'
    }

    fn redact(&self, text: &str) -> String {
        let Some(re) = &self.pattern else {
            return text.to_owned();
        };
        let mut out = String::with_capacity(text.len());
        let mut pos = 0usize;
        while pos <= text.len() {
            let Some(m) = re.find_at(text, pos) else {
                break;
            };
            let before_ok = text[..m.start()]
                .chars()
                .next_back()
                .is_none_or(|c| !Self::is_boundary_char(c));
            let after_ok = text[m.end()..]
                .chars()
                .next()
                .is_none_or(|c| !Self::is_boundary_char(c));
            if before_ok && after_ok {
                out.push_str(&text[pos..m.start()]);
                out.push_str("[private]");
                pos = m.end();
            } else {
                // Not a real boundary match: advance one character and retry,
                // so a longer marker starting later is still found.
                let step = text[m.start()..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&text[pos..m.start() + step]);
                pos = m.start() + step;
            }
        }
        out.push_str(&text[pos.min(text.len())..]);
        out
    }

    fn is_private_ref(&self, r: &Ref) -> bool {
        self.markers.contains(&r.iri)
    }

    fn clean_refs(&self, refs: &[Ref]) -> Vec<Ref> {
        refs.iter()
            .filter(|r| !self.is_private_ref(r))
            .map(|r| Ref {
                iri: self.redact(&r.iri),
                label: self.redact(&r.label),
            })
            .collect()
    }

    fn clean_record(&self, r: &ClassRecord) -> ClassRecord {
        let mut relations: IndexMap<&'static str, Vec<Ref>> = IndexMap::new();
        for (key, refs) in &r.relations {
            let cleaned = self.clean_refs(refs);
            if !cleaned.is_empty() {
                relations.insert(key, cleaned);
            }
        }
        ClassRecord {
            page_id: r.page_id.clone(),
            slug: self.redact(&r.slug),
            title: self.redact(&r.title),
            public: r.public,
            page_iri: self.redact(&r.page_iri),
            iri: self.redact(&r.iri),
            label: self.redact(&r.label),
            entity_type: r.entity_type,
            domain: self.redact(&r.domain),
            definition: self.redact(&r.definition),
            maturity: self.redact(&r.maturity),
            quality: r.quality,
            legacy_term_id: self.redact(&r.legacy_term_id),
            generated_by: r.generated_by.clone(),
            sub_class_of: self.clean_refs(&r.sub_class_of),
            instance_of: self.clean_refs(&r.instance_of),
            relations,
            links: self.clean_refs(&r.links),
            body: self.redact(&r.body),
            has_ontology: r.has_ontology,
            defines_as: self.clean_definition(&r.defines_as),
        }
    }

    /// A definition naming a private class is withheld **whole** (ADR-2124):
    /// dropping only the private conjunct would weaken the definition and
    /// classify more public classes under it than its author meant.
    fn clean_definition(&self, conjuncts: &[Conjunct]) -> Vec<Conjunct> {
        if conjuncts.iter().any(|c| self.is_private_ref(c.target())) {
            return Vec::new();
        }
        let clean = |r: &Ref| Ref {
            iri: self.redact(&r.iri),
            label: self.redact(&r.label),
        };
        conjuncts
            .iter()
            .map(|c| match c {
                Conjunct::Named(r) => Conjunct::Named(clean(r)),
                Conjunct::Some { relation, filler } => Conjunct::Some {
                    relation: relation.clone(),
                    filler: clean(filler),
                },
            })
            .collect()
    }
}

/// Project `corpus` onto the public set, dropping and redacting every private
/// reference.
///
/// `extra_private` carries identities from pages the walk excluded entirely.
///
/// # Errors
/// [`PublicationBlocked::Collision`] when an identity is claimed by both a
/// public and a private page.
#[allow(clippy::implicit_hasher)] // The build always uses the default hasher.
pub fn project(
    profile: &Profile,
    corpus: &Corpus,
    extra_private: &HashSet<String>,
) -> Result<Corpus, PublicationBlocked> {
    project_with_redactor(profile, corpus, extra_private).map(|(corpus, _)| corpus)
}

/// The marker redactor [`project`] applies to every public record, exposed so a
/// prose surface built from raw text (the markdown mirror) can apply exactly
/// the same masking after its own link rewriting.
#[derive(Debug)]
pub struct ProseRedactor(Redactor);

impl ProseRedactor {
    /// Compile a redactor over `markers` at [`REDACTOR_SIZE_LIMIT`].
    ///
    /// # Errors
    /// [`PublicationBlocked::Redactor`] when the alternation does not compile.
    /// The caller must refuse to publish: there is no safe fallback.
    pub fn new(markers: HashSet<String>) -> Result<Self, PublicationBlocked> {
        Self::with_size_limit(markers, REDACTOR_SIZE_LIMIT)
    }

    /// [`ProseRedactor::new`] with an explicit compiled-program size limit, so
    /// the refusal path can be exercised without a vault of millions of pages.
    ///
    /// # Errors
    /// As [`ProseRedactor::new`].
    pub fn with_size_limit(
        markers: HashSet<String>,
        size_limit: usize,
    ) -> Result<Self, PublicationBlocked> {
        Redactor::with_size_limit(markers, size_limit)
            .map(Self)
            .map_err(|e| PublicationBlocked::Redactor(e.to_string()))
    }

    /// Mask every private marker in `text` with `[private]`.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        self.0.redact(text)
    }
}

/// [`project`], also returning the redactor it used.
///
/// # Errors
/// [`PublicationBlocked::Collision`] when an identity is claimed by both a
/// public and a private page; [`PublicationBlocked::Redactor`] when the
/// private-marker alternation does not compile.
#[allow(clippy::implicit_hasher)] // The build always uses the default hasher.
pub fn project_with_redactor(
    profile: &Profile,
    corpus: &Corpus,
    extra_private: &HashSet<String>,
) -> Result<(Corpus, ProseRedactor), PublicationBlocked> {
    let split = Split::checked(&corpus.records, extra_private)?;
    let redactor = split.redactor(profile)?;
    let records: Vec<ClassRecord> = split
        .public
        .iter()
        .map(|r| redactor.clean_record(r))
        .collect();
    Ok((corpus_of(records), ProseRedactor(redactor)))
}

/// A [`Corpus`] over `records`, with the class-slug index the projection
/// builds: only records that declare an ontology identity are keyed.
fn corpus_of(records: Vec<ClassRecord>) -> Corpus {
    let mut by_class_slug = IndexMap::new();
    for (i, r) in records.iter().enumerate() {
        if r.has_ontology && !r.iri.is_empty() {
            by_class_slug.entry(r.class_slug()).or_insert(i);
        }
    }
    Corpus {
        records,
        by_class_slug,
    }
}

/// Records divided into the public and the private, once the identity check
/// has passed.
struct Split<'a> {
    public: Vec<&'a ClassRecord>,
    private: Vec<&'a ClassRecord>,
    private_ids: HashSet<String>,
}

impl<'a> Split<'a> {
    /// Divide `records` and refuse when one identity (page id, slug, page IRI
    /// or class IRI) is claimed on both sides. `extra_private` carries the
    /// identities of held pages, which count as private whatever their flag.
    fn checked(
        records: &'a [ClassRecord],
        extra_private: &HashSet<String>,
    ) -> Result<Self, PublicationBlocked> {
        let (public, private): (Vec<&ClassRecord>, Vec<&ClassRecord>) =
            records.iter().partition(|r| r.public);

        let public_ids: HashSet<String> = public.iter().flat_map(|r| identity(r)).collect();
        let mut private_ids: HashSet<String> = private.iter().flat_map(|r| identity(r)).collect();
        private_ids.extend(extra_private.iter().cloned());

        if let Some(clash) = public_ids.intersection(&private_ids).next() {
            return Err(PublicationBlocked::Collision(clash.clone()));
        }
        Ok(Self {
            public,
            private,
            private_ids,
        })
    }

    /// The marker redactor for this division: every private identity, every
    /// private name no public page also carries, and the published web form of
    /// each private identity.
    fn redactor(&self, profile: &Profile) -> Result<Redactor, PublicationBlocked> {
        let names = |records: &[&ClassRecord]| -> HashSet<String> {
            records
                .iter()
                .flat_map(|r| [r.title.clone(), r.label.clone()])
                .filter(|s| !s.is_empty())
                .collect()
        };
        let public_names = names(&self.public);
        let private_names = names(&self.private);

        let mut markers: HashSet<String> = self.private_ids.clone();
        markers.extend(private_names.difference(&public_names).cloned());
        for value in &self.private_ids {
            // A redacted URN must not reappear as a resolvable web IRI.
            if let Some(published) = profile.iris.project(value) {
                markers.insert(published);
            }
        }
        // An identity that is also a public name must not be redacted out of the
        // public bundle; the collision check already proved no page claims
        // both, so this only strips names that coincide.
        for name in &public_names {
            markers.remove(name);
        }
        Redactor::new(markers).map_err(|e| PublicationBlocked::Redactor(e.to_string()))
    }
}

/// Build the public projection using the same cross-vault link resolution as
/// the Markdown staging. Private targets are removed before resolution and
/// closure, including aliases, fragments and targets in the sibling vault.
///
/// Definitions containing a private conjunct are withheld whole; removing one
/// conjunct would weaken the author's definition. The original corpus is
/// checked first so rewriting cannot conceal an identity collision.
///
/// # Errors
/// [`PublicationBlocked::Collision`] for conflicting publication identities.
pub fn project_vault_with_links(
    profile: &Profile,
    vault: &Vault,
    vocab: &Vocabulary,
    extra_private: &HashSet<String>,
    links: &crate::build::redact::PrivateLinks,
) -> Result<(Corpus, ProseRedactor), PublicationBlocked> {
    project(profile, &Corpus::build(vault, vocab), extra_private)?;
    let cleaned = clean_vault(vault, links);
    project_with_redactor(profile, &Corpus::build(&cleaned, vocab), extra_private)
}

/// `vault` as the public surfaces may see it: held pages forced private, and
/// every public page's curated links, relation targets, `defines-as` and body
/// stripped of whatever `links` resolves to a non-public page. Links resolve
/// from the page's own vault (`vault.kind`), then the other.
fn clean_vault(vault: &Vault, links: &crate::build::redact::PrivateLinks) -> Vault {
    let mut cleaned = vault.clone();
    for page in &mut cleaned.pages {
        let rel = page
            .rel_path
            .strip_prefix("pages")
            .unwrap_or(&page.rel_path);
        if crate::build::publish::held_back(rel) {
            page.frontmatter.map.insert("public".into(), false.into());
        }
        if !page.is_public() {
            continue;
        }
        // Pin the curated set, including an empty set: removing `links` would
        // silently switch the record back to a scan of all body wikilinks.
        let public_links: Vec<serde_yaml::Value> = page
            .outbound_links()
            .into_iter()
            .filter(|w| !links.is_private(vault.kind, &w.target))
            .map(|w| serde_yaml::Value::String(w.to_string()))
            .collect();
        page.frontmatter.map.retain(|key, value| {
            if key.as_str() == Some(crate::input::definition::DEFINES_AS_KEY)
                && links.value_has_private_target(vault.kind, value)
            {
                return false;
            }
            if let Some(clean) = links.frontmatter_value(vault.kind, value) {
                *value = clean;
                true
            } else {
                false
            }
        });
        page.frontmatter
            .map
            .insert("links".into(), public_links.into());
        page.body = links.body(vault.kind, &page.body, &mut Default::default());
    }
    cleaned
}

/// `vault` with its held pages forced private and nothing else changed: the
/// raw view the identity check reads, so a page the publish staging holds back
/// is not counted as public.
fn hold_back(vault: &Vault) -> Vault {
    let mut held = vault.clone();
    for page in &mut held.pages {
        let rel = page
            .rel_path
            .strip_prefix("pages")
            .unwrap_or(&page.rel_path);
        if crate::build::publish::held_back(rel) {
            page.frontmatter.map.insert("public".into(), false.into());
        }
    }
    held
}

/// The publish slug of a `working/` page in the machine bundle: `working/`
/// plus the page id with each path segment slugified, subdirectories kept so
/// two `Foo.md` in different folders do not share one.
#[must_use]
pub fn working_slug(page_id: &str) -> String {
    let segments: Vec<String> = page_id.split('/').map(slugify).collect();
    format!("working/{}", segments.join("/"))
}

/// The machine scope's projection: the knowledge projection exactly as the
/// public build makes it, plus `working/`'s public pages as plain records.
#[derive(Debug)]
pub struct MachineProjection {
    /// The knowledge projection, byte for byte what a public build projects.
    /// Every ontology artefact is emitted from it.
    pub knowledge: Corpus,
    /// The redactor of the knowledge projection, for the surfaces that carry
    /// knowledge prose.
    pub redactor: ProseRedactor,
    /// The plain `working/` records, already redacted and renamed.
    pub plain: Vec<ClassRecord>,
    /// Public `working/` pages dropped because a public knowledge page has the
    /// same identity.
    pub yielded: usize,
}

impl MachineProjection {
    /// The knowledge records followed by the plain ones: the corpus the page
    /// API and the search index are built from.
    #[must_use]
    pub fn merged(&self) -> Corpus {
        let mut records = self.knowledge.records.clone();
        records.extend(self.plain.iter().cloned());
        Corpus {
            records,
            by_class_slug: self.knowledge.by_class_slug.clone(),
        }
    }
}

/// Project `knowledge` as a public build does and add `working`'s public pages
/// as plain records (`--scope machine`).
///
/// * The knowledge side is not touched by `working/`: it takes the same
///   identity check, the same marker set and the same link cleaning as in a
///   public build, so every ontology artefact keeps its bytes.
/// * The identity check and the marker set cover **both** vaults, on identities
///   as authored (before the `working/` prefix). The twin rule is directional:
///   a public working page beside a private page of either vault with the same
///   identity is a [`PublicationBlocked::Collision`]; a public working page
///   whose identity a public knowledge page already holds *yields* to it; and a
///   private working page whose identity a public knowledge page holds is
///   harmless and yields silently (it is never published and adds no marker).
/// * A working record's links resolve through `links`, the resolver redaction
///   uses, to the target's published slug.
/// * What survives is renamed under `working/` and stripped of any ontology
///   identity.
///
/// # Errors
/// [`PublicationBlocked::Collision`] or [`PublicationBlocked::Redactor`], as
/// [`project_vault_with_links`].
pub fn project_machine(
    profile: &Profile,
    knowledge: &Vault,
    working: &Vault,
    vocab: &Vocabulary,
    extra_private: &HashSet<String>,
    links: &crate::build::redact::PrivateLinks,
) -> Result<MachineProjection, PublicationBlocked> {
    use crate::input::page::VaultKind;

    let (knowledge_corpus, redactor) =
        project_vault_with_links(profile, knowledge, vocab, extra_private, links)?;

    // The union's extra identities: the held pages of both vaults.
    let mut union_extra = extra_private.clone();
    union_extra.extend(excluded_identities_of(working));

    // Identity check over the authored records, before any rewriting could
    // conceal a clash. The twin rule is directional: a PRIVATE working page
    // whose identity a PUBLIC knowledge page holds is harmless (nothing of it
    // is published) and yields silently; a PUBLIC working page beside a
    // PRIVATE page of either vault would expose the private identity through
    // its own, and is refused.
    let knowledge_raw = Corpus::build(knowledge, vocab).records;
    let public_knowledge: HashSet<String> = knowledge_raw
        .iter()
        .filter(|r| r.public)
        .flat_map(identity)
        .collect();
    let mut raw = knowledge_raw;
    raw.extend(
        Corpus::build(&hold_back(working), vocab)
            .records
            .into_iter()
            .filter(|r| r.public || !identity(r).iter().any(|id| public_knowledge.contains(id))),
    );
    Split::checked(&raw, &union_extra)?;

    let cleaned_knowledge = Corpus::build(&clean_vault(knowledge, links), vocab);
    let cleaned_working = clean_vault(working, links);
    let mut working_corpus = Corpus::build(&cleaned_working, vocab);

    // A public working page whose identity a public knowledge page already
    // holds yields to it (knowledge wins, as in `VisionClaw`'s merge). Decided
    // here, on identities as authored, so the links below can honour it.
    let knowledge_identities: HashSet<String> = cleaned_knowledge
        .records
        .iter()
        .filter(|r| r.public)
        .flat_map(identity)
        .collect();
    let yields: HashSet<usize> = working_corpus
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.public
                && identity(r)
                    .iter()
                    .any(|id| knowledge_identities.contains(id))
        })
        .map(|(i, _)| i)
        .collect();
    let yielded_ids: HashSet<String> = yields
        .iter()
        .filter_map(|i| working_corpus.records.get(*i).map(|r| r.page_id.clone()))
        .collect();

    // A working page's curated links point at the target's published slug:
    // the post-disambiguation slug for a knowledge page, the `working/` slug
    // for a working one. A target that resolves to nothing, or to a page that
    // yielded and so has no record, stays slug-verbatim, as a knowledge page's
    // does.
    for (page, record) in cleaned_working
        .pages
        .iter()
        .zip(&mut working_corpus.records)
    {
        if !record.public {
            continue;
        }
        for (link, target) in page.outbound_links().into_iter().zip(&mut record.links) {
            let slug = match links.resolve_public(VaultKind::Working, &link.target) {
                Some((VaultKind::Knowledge, i)) => {
                    cleaned_knowledge.records.get(i).map(|r| r.slug.clone())
                }
                Some((VaultKind::Working, i)) if !yields.contains(&i) => {
                    working.pages.get(i).map(|p| working_slug(&p.id))
                }
                _ => None,
            };
            if let Some(slug) = slug {
                target.iri = format!("{}{slug}", vocab.namespace);
            }
        }
    }

    let knowledge_len = cleaned_knowledge.records.len();
    let mut union = cleaned_knowledge.records;
    // Private working twins of public knowledge pages were cleared above;
    // they contribute no marker either, since their name is a public one.
    union.extend(working_corpus.records.into_iter().filter(|r| {
        r.public
            || !identity(r)
                .iter()
                .any(|id| knowledge_identities.contains(id))
    }));
    let split = Split::checked(&union, &union_extra)?;
    let union_redactor = split.redactor(profile)?;

    let mut plain = Vec::new();
    for record in &union[knowledge_len..] {
        if !record.public || yielded_ids.contains(&record.page_id) {
            continue;
        }
        let mut record = union_redactor.clean_record(record);
        // D2: the machine-scope page IRI is the page namespace plus the
        // working slug, so it carries no spaces and matches the record's slug.
        record.slug = working_slug(&record.page_id);
        record.page_iri = format!("{}{}", profile.iris.page_namespace, record.slug);
        record.page_id = format!("working/{}", record.page_id);
        record.has_ontology = false;
        record.iri.clear();
        record.entity_type = EntityType::Class;
        plain.push(record);
    }
    let yielded = yields.len();

    Ok(MachineProjection {
        knowledge: knowledge_corpus,
        redactor,
        plain,
        yielded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::model::EntityType;

    #[test]
    fn redactor_refuses_when_the_alternation_exceeds_the_size_limit() {
        let markers: HashSet<String> = (0..64).map(|i| format!("Private Programme {i}")).collect();
        let err = ProseRedactor::with_size_limit(markers.clone(), 16).unwrap_err();
        assert!(matches!(err, PublicationBlocked::Redactor(_)), "{err}");
        let ok = ProseRedactor::new(markers).unwrap();
        assert_eq!(
            ok.redact("see Private Programme 7 here"),
            "see [private] here"
        );
    }

    #[test]
    fn an_empty_marker_set_compiles_to_no_pattern() {
        let r = ProseRedactor::new(HashSet::new()).unwrap();
        assert_eq!(r.redact("untouched"), "untouched");
    }

    fn record(id: &str, public: bool) -> ClassRecord {
        ClassRecord {
            page_id: id.to_owned(),
            slug: crate::input::slug::slugify(id),
            title: id.to_owned(),
            public,
            page_iri: format!("urn:visionflow:page:{}", crate::input::slug::slugify(id)),
            iri: format!("urn:ngm:class:{}", crate::input::slug::slugify(id)),
            label: id.to_owned(),
            entity_type: EntityType::Class,
            domain: String::new(),
            definition: String::new(),
            maturity: "draft".into(),
            quality: 0.0,
            legacy_term_id: String::new(),
            generated_by: None,
            sub_class_of: Vec::new(),
            instance_of: Vec::new(),
            relations: IndexMap::new(),
            links: Vec::new(),
            body: String::new(),
            has_ontology: true,
            defines_as: Vec::new(),
        }
    }

    fn corpus_of(records: Vec<ClassRecord>) -> Corpus {
        let mut by_class_slug = IndexMap::new();
        for (i, r) in records.iter().enumerate() {
            by_class_slug.entry(r.class_slug()).or_insert(i);
        }
        Corpus {
            records,
            by_class_slug,
        }
    }

    #[test]
    fn keeps_only_public_pages() {
        let c = corpus_of(vec![record("Pub", true), record("Secret", false)]);
        let p = project(&crate::profile::test_estate(), &c, &HashSet::new()).unwrap();
        assert_eq!(p.records.len(), 1);
        assert_eq!(p.records[0].page_id, "Pub");
    }

    #[test]
    fn drops_edges_that_point_at_a_private_page() {
        let mut pub_rec = record("Pub", true);
        pub_rec.sub_class_of = vec![
            Ref {
                iri: "urn:ngm:class:secret".into(),
                label: "Secret".into(),
            },
            Ref {
                iri: "urn:ngm:class:open".into(),
                label: "Open".into(),
            },
        ];
        let c = corpus_of(vec![pub_rec, record("Secret", false), record("Open", true)]);
        let p = project(&crate::profile::test_estate(), &c, &HashSet::new()).unwrap();
        let parents: Vec<&str> = p.records[0]
            .sub_class_of
            .iter()
            .map(|r| r.iri.as_str())
            .collect();
        assert_eq!(parents, vec!["urn:ngm:class:open"]);
    }

    /// ADR-2124: a definition is never weakened by projection. One private
    /// conjunct withholds the whole definition; an all-public one survives.
    #[test]
    fn a_definition_naming_a_private_page_is_withheld_whole() {
        let r = |slug: &str| Ref {
            iri: format!("urn:ngm:class:{slug}"),
            label: slug.to_owned(),
        };
        let mut leaky = record("Leaky", true);
        leaky.defines_as = vec![
            Conjunct::Named(r("open")),
            Conjunct::Some {
                relation: "has-part".into(),
                filler: r("secret"),
            },
        ];
        let mut clean = record("Clean", true);
        clean.defines_as = vec![
            Conjunct::Named(r("open")),
            Conjunct::Some {
                relation: "has-part".into(),
                filler: r("open"),
            },
        ];
        let c = corpus_of(vec![
            leaky,
            clean.clone(),
            record("Secret", false),
            record("Open", true),
        ]);
        let p = project(&crate::profile::test_estate(), &c, &HashSet::new()).unwrap();
        assert!(p.records[0].defines_as.is_empty(), "{:?}", p.records[0]);
        assert_eq!(p.records[1].defines_as, clean.defines_as);
    }

    #[test]
    fn redacts_a_private_name_out_of_prose() {
        let mut pub_rec = record("Pub", true);
        pub_rec.body = "Refer to Secret Project for detail; SecretProjectX is unrelated.".into();
        let c = corpus_of(vec![pub_rec, record("Secret Project", false)]);
        let p = project(&crate::profile::test_estate(), &c, &HashSet::new()).unwrap();
        assert_eq!(
            p.records[0].body,
            "Refer to [private] for detail; SecretProjectX is unrelated."
        );
    }

    #[test]
    fn a_name_that_is_also_public_is_not_redacted() {
        let mut pub_rec = record("Pub", true);
        pub_rec.body = "Shared appears here.".into();
        let c = corpus_of(vec![pub_rec, record("Shared", true), {
            let mut r = record("Other", false);
            r.title = "Shared".into();
            r.label = "Shared".into();
            r
        }]);
        let p = project(&crate::profile::test_estate(), &c, &HashSet::new()).unwrap();
        assert_eq!(p.records[0].body, "Shared appears here.");
    }

    #[test]
    fn the_http_projection_of_a_private_urn_is_redacted_too() {
        let mut pub_rec = record("Pub", true);
        pub_rec.body = "See https://narrativegoldmine.com/class/secret now.".into();
        let c = corpus_of(vec![pub_rec, record("Secret", false)]);
        let p = project(&crate::profile::test_estate(), &c, &HashSet::new()).unwrap();
        assert!(p.records[0].body.contains("[private]"));
    }

    #[test]
    fn an_identity_collision_blocks_the_build() {
        let mut a = record("A", true);
        let mut b = record("B", false);
        a.iri = "urn:ngm:class:same".into();
        b.iri = "urn:ngm:class:same".into();
        let err = project(
            &crate::profile::test_estate(),
            &corpus_of(vec![a, b]),
            &HashSet::new(),
        )
        .unwrap_err();
        assert!(matches!(err, PublicationBlocked::Collision(_)));
    }

    #[test]
    fn excluded_identities_are_treated_as_private() {
        let mut pub_rec = record("Pub", true);
        pub_rec.links = vec![Ref {
            iri: "urn:ngm:class:held".into(),
            label: "Held".into(),
        }];
        let extra: HashSet<String> = ["urn:ngm:class:held".to_owned()].into_iter().collect();
        let p = project(
            &crate::profile::test_estate(),
            &corpus_of(vec![pub_rec]),
            &extra,
        )
        .unwrap();
        assert_eq!(p.records[0].links, []);
    }

    #[test]
    fn a_working_slug_keeps_its_folders_and_slugifies_each_segment() {
        assert_eq!(
            working_slug("Fixture Public Note"),
            "working/fixture-public-note"
        );
        assert_eq!(working_slug("a/Foo Bar"), "working/a/foo-bar");
    }

    #[test]
    fn machine_projection_masks_private_working_names_in_plain_prose() {
        use crate::input::page::{Page, Vault, VaultKind};
        let vault = |kind: VaultKind, pages: &[(&str, &str)]| Vault {
            root: std::path::PathBuf::new(),
            kind,
            journals: Vec::new(),
            skipped: Vec::new(),
            pages: pages
                .iter()
                .map(|(id, text)| {
                    Page::parse(
                        format!("/v/pages/{id}.md"),
                        format!("pages/{id}.md"),
                        *id,
                        text,
                    )
                    .unwrap()
                })
                .collect(),
        };
        let knowledge = vault(VaultKind::Knowledge, &[]);
        let working = vault(
            VaultKind::Working,
            &[
                (
                    "Open",
                    "---\ntype: Note\npublic: true\n---\nRefer to Secret Project for detail.\n",
                ),
                ("Secret Project", "---\ntype: Note\n---\nHidden.\n"),
            ],
        );
        let vocab =
            Vocabulary::from_yaml_str("version: 1\nnamespace: \"urn:ngm:class:\"\n").unwrap();
        let links = crate::build::redact::PrivateLinks::new(&[&knowledge, &working]);
        let projected = project_machine(
            &crate::profile::test_estate(),
            &knowledge,
            &working,
            &vocab,
            &HashSet::new(),
            &links,
        )
        .unwrap();
        assert_eq!(projected.plain.len(), 1);
        let record = &projected.plain[0];
        assert_eq!(record.body.trim(), "Refer to [private] for detail.");
        assert_eq!(record.page_id, "working/Open");
        assert_eq!(record.slug, "working/open");
        assert!(!record.has_ontology && record.iri.is_empty());
        assert_eq!(projected.merged().by_class_slug.len(), 0);
    }

    #[test]
    fn census_counts_and_balances() {
        let dir = tempfile::tempdir().unwrap();
        let pages = dir.path().join("pages");
        std::fs::create_dir_all(pages.join("_misc")).unwrap();
        std::fs::write(pages.join("A.md"), "---\npublic: true\n---\n").unwrap();
        std::fs::write(pages.join("B.md"), "---\npublic: false\n---\n").unwrap();
        std::fs::write(pages.join("C.md"), "no frontmatter\n").unwrap();
        std::fs::write(pages.join("_misc/D.md"), "---\npublic: true\n---\n").unwrap();
        let census = inspect_inputs(&pages).unwrap();
        assert_eq!(census.public_pages, 1);
        assert_eq!(census.non_public_pages, 1);
        assert_eq!(census.excluded, 2);
        assert!(census.balanced);
    }

    #[test]
    fn a_string_public_flag_refuses_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let pages = dir.path().join("pages");
        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(pages.join("A.md"), "---\npublic: \"true\"\n---\n").unwrap();
        assert!(matches!(
            inspect_inputs(&pages),
            Err(PublicationBlocked::Census(_))
        ));
    }
}
