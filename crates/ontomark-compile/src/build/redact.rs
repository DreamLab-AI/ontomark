//! Private-link resolution shared by every public export surface (EXP-04).
//!
//! The public projection removes private typed and curated references before
//! closure and rewrites body links using this resolver. That includes private
//! aliases, headings and sibling-vault targets in Turtle, page APIs, indexes,
//! OKF and NGG1. `publish/` stages the author's own
//! Markdown, and the `api/markdown` mirror carries the body. Both used to carry
//! `[[Wikilinks]]` and frontmatter relation entries that name pages which are
//! **not public**, and a link names its target in clear text.
//!
//! This module closes that. A link is *redacted* when its target resolves, by
//! the build's own name rules (`NameIndex`: page id, title, slug and aliases,
//! case-insensitively, first page wins), to an existing page that is not
//! public. A target that resolves to nothing names no private page and is left
//! alone, as is a target that resolves to a public page.
//!
//! "Not public" means no frontmatter, `public` not a literal true, a page in a
//! folder the publish staging holds back (`_misc`, `misc`), or a journal.
//! Resolution looks in the page's own vault first and then the other vault, the
//! way a link in `working/` reaches a `knowledge/` page.
//!
//! # Rules
//!
//! Frontmatter ([`PrivateLinks::frontmatter`]), edited line by line so every
//! other byte survives:
//!
//! * a list entry that is exactly one link to a non-public page is removed;
//! * a list left empty loses its key line too;
//! * a scalar that is exactly one such link loses its key;
//! * a link embedded in longer text is rewritten as in the body.
//!
//! Body ([`PrivateLinks::body`]):
//!
//! * `[[Target|Shown]]` becomes `Shown`, unless `Shown` is the target's title,
//!   id, slug or one of its aliases (or contains any of those names), in which
//!   case it becomes the neutral marker [`MARKER`];
//! * `[[Target]]` and `[[Target#Heading]]` become [`MARKER`];
//! * an embed `![[Target]]` is removed entirely;
//! * the private title is never emitted.
//!
//! Text inside fenced code blocks and inline code spans is authored text, not a
//! link, and is left byte-for-byte as written. Line endings (LF or CRLF) are
//! preserved.
//!
//! Not covered: ordinary Markdown links (`[text](Page.md)`), and bare mentions
//! of a private title in prose (the projection's marker redactor masks those in
//! the mirror; the staged `publish/` prose is the author's).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_yaml::Value as Yaml;

use crate::build::publish::held_back;
use crate::input::frontmatter::Wikilink;
use crate::input::page::{Page, Vault, VaultKind};
use crate::ir::model::NameIndex;

/// What replaces a link whose target is private and which has no safe label.
pub const MARKER: &str = "(private link)";

/// Counts of what one redaction pass changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tally {
    /// Frontmatter list entries and scalar values removed.
    pub frontmatter_entries_removed: usize,
    /// Frontmatter keys removed because nothing was left under them.
    pub frontmatter_keys_removed: usize,
    /// Links embedded in longer frontmatter text that were rewritten.
    pub frontmatter_inline_rewrites: usize,
    /// Body links rewritten to a label or to [`MARKER`].
    pub body_links_rewritten: usize,
    /// Of those, the ones that kept the author's own display text.
    pub body_labels_kept: usize,
    /// Body embeds (`![[…]]`) removed.
    pub embeds_removed: usize,
}

impl Tally {
    /// `true` when the pass changed anything.
    #[must_use]
    pub fn changed(&self) -> bool {
        *self != Self::default()
    }

    /// Add `other` into `self`.
    pub fn add(&mut self, other: &Tally) {
        self.frontmatter_entries_removed += other.frontmatter_entries_removed;
        self.frontmatter_keys_removed += other.frontmatter_keys_removed;
        self.frontmatter_inline_rewrites += other.frontmatter_inline_rewrites;
        self.body_links_rewritten += other.body_links_rewritten;
        self.body_labels_kept += other.body_labels_kept;
        self.embeds_removed += other.embeds_removed;
    }
}

/// Totals for one surface (a vault's `publish/` pages, or the mirror).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SurfaceRedaction {
    /// Public pages whose output differs from the source.
    pub pages_changed: usize,
    /// What changed across them.
    #[serde(flatten)]
    pub tally: Tally,
}

impl SurfaceRedaction {
    /// Fold one page's tally in.
    pub fn record(&mut self, page: &Tally) {
        if page.changed() {
            self.pages_changed += 1;
        }
        self.tally.add(page);
    }
}

/// What redaction did in one build.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RedactionReport {
    /// The `publish/` staging, by vault name.
    pub publish: BTreeMap<String, SurfaceRedaction>,
    /// The `api/markdown` mirror (empty when it was not written).
    pub mirror: SurfaceRedaction,
}

/// A page as the resolver sees it.
#[derive(Debug, Clone)]
struct Entry {
    private: bool,
    id: String,
    title: String,
    slug: String,
    aliases: Vec<String>,
}

#[derive(Debug)]
struct VaultIndex {
    kind: VaultKind,
    names: NameIndex,
    entries: Vec<Entry>,
}

/// The outcome of resolving a link target.
#[derive(Debug, Clone, Copy)]
enum Resolution<'a> {
    /// A public page: its vault and its position in [`Vault::pages`].
    Public(VaultKind, usize),
    Private(&'a Entry),
    Unresolved,
}

/// Resolves link targets to public/private, and rewrites text accordingly.
#[derive(Debug)]
pub struct PrivateLinks {
    vaults: Vec<VaultIndex>,
}

impl PrivateLinks {
    /// Index every page (and journal) of the given vaults.
    #[must_use]
    pub fn new(vaults: &[&Vault]) -> Self {
        let vaults = vaults
            .iter()
            .map(|vault| {
                let regular = vault
                    .pages
                    .iter()
                    .map(|p| (p, !p.is_public() || held_back(&p.rel_path)));
                // Journals are never published: a link to one is a link to a
                // non-public page. Regular pages come first so they win a clash.
                let journals = vault.journals.iter().map(|p| (p, true));
                let all: Vec<(&Page, bool)> = regular.chain(journals).collect();
                VaultIndex {
                    kind: vault.kind,
                    names: NameIndex::build_iter(all.iter().map(|(p, _)| *p)),
                    entries: all
                        .iter()
                        .map(|(p, private)| Entry {
                            private: *private,
                            id: p.id.clone(),
                            title: p.title(),
                            slug: p.slug(),
                            aliases: p.frontmatter.strings("aliases"),
                        })
                        .collect(),
                }
            })
            .collect();
        Self { vaults }
    }

    fn lookup(&self, home: VaultKind, name: &str) -> Resolution<'_> {
        let own = self.vaults.iter().filter(|v| v.kind == home);
        let others = self.vaults.iter().filter(|v| v.kind != home);
        for v in own.chain(others) {
            if let Some(i) = v.names.get(name) {
                let e = &v.entries[i];
                return if e.private {
                    Resolution::Private(e)
                } else {
                    Resolution::Public(v.kind, i)
                };
            }
        }
        Resolution::Unresolved
    }

    /// Resolve a wikilink target as written: whole, then without a `#heading`
    /// or `#^block` fragment, then without a `.md` suffix.
    fn resolve(&self, home: VaultKind, target: &str) -> Resolution<'_> {
        let target = target.trim();
        if target.is_empty() {
            return Resolution::Unresolved;
        }
        let whole = self.lookup(home, target);
        if !matches!(whole, Resolution::Unresolved) {
            return whole;
        }
        let base = target.split_once('#').map_or(target, |(b, _)| b).trim();
        if base.is_empty() {
            return Resolution::Unresolved; // `[[#Heading]]` is a link within the page
        }
        let by_base = self.lookup(home, base);
        if !matches!(by_base, Resolution::Unresolved) {
            return by_base;
        }
        base.strip_suffix(".md")
            .map_or(Resolution::Unresolved, |b| self.lookup(home, b.trim()))
    }

    /// The public page `target` names, as written in a page of the `home`
    /// vault: its vault and its position in that vault's [`Vault::pages`].
    ///
    /// This is the one resolver. Redaction decides on it whether a link is
    /// private, and the machine scope's plain records use it to point their
    /// links at the pages' published slugs, so the two can never disagree about
    /// which page a link means. `None` for a private or unresolved target.
    #[must_use]
    pub(crate) fn resolve_public(
        &self,
        home: VaultKind,
        target: &str,
    ) -> Option<(VaultKind, usize)> {
        match self.resolve(home, target) {
            Resolution::Public(kind, index) => Some((kind, index)),
            Resolution::Private(_) | Resolution::Unresolved => None,
        }
    }

    /// `true` when `target` resolves to a page that is not public.
    #[must_use]
    pub fn is_private(&self, home: VaultKind, target: &str) -> bool {
        matches!(self.resolve(home, target), Resolution::Private(_))
    }

    /// Whether a structured frontmatter value contains a private link target.
    /// Used to withhold a `defines-as` conjunction whole.
    #[must_use]
    pub fn value_has_private_target(&self, home: VaultKind, value: &Yaml) -> bool {
        match value {
            Yaml::String(s) => self.whole_private_link(home, s),
            Yaml::Sequence(values) => values
                .iter()
                .any(|v| self.value_has_private_target(home, v)),
            Yaml::Mapping(values) => values
                .values()
                .any(|v| self.value_has_private_target(home, v)),
            Yaml::Tagged(value) => self.value_has_private_target(home, &value.value),
            _ => false,
        }
    }

    /// Clean a machine frontmatter value before constructing the corpus.
    /// Whole private links are dropped; prose links use the Markdown rules.
    #[must_use]
    pub fn frontmatter_value(&self, home: VaultKind, value: &Yaml) -> Option<Yaml> {
        match value {
            Yaml::String(s) if self.whole_private_link(home, s) => None,
            Yaml::String(s) => Some(self.body(home, s, &mut Tally::default()).into()),
            Yaml::Sequence(values) => {
                let kept: Vec<Yaml> = values
                    .iter()
                    .filter_map(|v| self.frontmatter_value(home, v))
                    .collect();
                (!kept.is_empty()).then_some(Yaml::Sequence(kept))
            }
            Yaml::Mapping(values) => {
                let kept: serde_yaml::Mapping = values
                    .iter()
                    .filter_map(|(k, v)| self.frontmatter_value(home, v).map(|v| (k.clone(), v)))
                    .collect();
                (!kept.is_empty()).then_some(Yaml::Mapping(kept))
            }
            Yaml::Tagged(value) => {
                let mut clean = value.clone();
                clean.value = self.frontmatter_value(home, &value.value)?;
                Some(Yaml::Tagged(clean))
            }
            _ => Some(value.clone()),
        }
    }

    // ---------------------------------------------------------------- body

    /// Redact a Markdown body (or any prose) written in the `home` vault.
    #[must_use]
    pub fn body(&self, home: VaultKind, text: &str, tally: &mut Tally) -> String {
        let mut out = String::with_capacity(text.len());
        let mut fence: Option<(char, usize)> = None;
        for line in text.split_inclusive('\n') {
            let (content, eol) = split_eol(line);
            if let Some((ch, len)) = fence {
                out.push_str(line);
                if closes_fence(content, ch, len) {
                    fence = None;
                }
                continue;
            }
            if let Some(opened) = opens_fence(content) {
                fence = Some(opened);
                out.push_str(line);
                continue;
            }
            out.push_str(&self.inline(home, content, tally, false));
            out.push_str(eol);
        }
        out
    }

    /// Rewrite the links on one line, leaving inline code spans alone.
    /// `frontmatter` selects the counter the rewrites are credited to.
    fn inline(&self, home: VaultKind, line: &str, tally: &mut Tally, frontmatter: bool) -> String {
        if !line.contains("[[") {
            return line.to_owned();
        }
        let mut out = String::with_capacity(line.len());
        let mut i = 0;
        while i < line.len() {
            let rest = &line[i..];
            if rest.starts_with('`') {
                let n = rest.bytes().take_while(|b| *b == b'`').count();
                if let Some(end) = find_run(&rest[n..], n) {
                    let stop = n + end + n;
                    out.push_str(&rest[..stop]);
                    i += stop;
                } else {
                    out.push_str(&rest[..n]);
                    i += n;
                }
                continue;
            }
            let embed = rest.starts_with("![[");
            if embed || rest.starts_with("[[") {
                let open = if embed { 3 } else { 2 };
                if let Some(close) = rest[open..].find("]]") {
                    let inner = &rest[open..open + close];
                    let total = open + close + 2;
                    if let Some(replacement) = self.rewrite(home, inner, embed, tally, frontmatter)
                    {
                        out.push_str(&replacement);
                    } else {
                        out.push_str(&rest[..total]);
                    }
                    i += total;
                    continue;
                }
            }
            let ch = rest.chars().next().unwrap_or('\u{fffd}');
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    /// The replacement for one link, or `None` to leave it as written.
    fn rewrite(
        &self,
        home: VaultKind,
        inner: &str,
        embed: bool,
        tally: &mut Tally,
        frontmatter: bool,
    ) -> Option<String> {
        let (target, shown) = split_link(inner);
        let Resolution::Private(entry) = self.resolve(home, target) else {
            return None;
        };
        if embed {
            if frontmatter {
                tally.frontmatter_inline_rewrites += 1;
            } else {
                tally.embeds_removed += 1;
            }
            return Some(String::new());
        }
        let label = shown.filter(|s| safe_label(s, entry));
        if frontmatter {
            tally.frontmatter_inline_rewrites += 1;
        } else {
            tally.body_links_rewritten += 1;
            if label.is_some() {
                tally.body_labels_kept += 1;
            }
        }
        Some(label.unwrap_or(MARKER).to_owned())
    }

    // --------------------------------------------------------- frontmatter

    /// Redact a rendered frontmatter block (the `---` fences included).
    #[must_use]
    pub fn frontmatter(&self, home: VaultKind, text: &str, tally: &mut Tally) -> String {
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < lines.len() {
            if !is_key_line(lines[i]) {
                out.push_str(&self.inline_keep_eol(home, lines[i], tally));
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < lines.len() && !is_key_line(lines[j]) && !is_fence_line(lines[j]) {
                j += 1;
            }
            self.block(home, &lines[i..j], &mut out, tally);
            i = j;
        }
        out
    }

    fn inline_keep_eol(&self, home: VaultKind, line: &str, tally: &mut Tally) -> String {
        let (content, eol) = split_eol(line);
        let mut s = self.inline(home, content, tally, true);
        s.push_str(eol);
        s
    }

    /// One top-level key and everything indented or listed under it.
    fn block(&self, home: VaultKind, block: &[&str], out: &mut String, tally: &mut Tally) {
        let (key_content, _) = split_eol(block[0]);
        let after_key = key_content.split_once(':').map_or("", |(_, v)| v).trim();

        if key_content.split_once(':').is_some_and(|(k, _)| {
            let k = k.trim();
            k == "defines-as"
                || (k.starts_with(['\"', '\''])
                    && serde_yaml::from_str::<String>(k).ok().as_deref() == Some("defines-as"))
        }) {
            if let Ok(Yaml::Mapping(map)) = serde_yaml::from_str::<Yaml>(&block.concat()) {
                if map.values().any(|v| self.value_has_private_target(home, v)) {
                    tally.frontmatter_keys_removed += 1;
                    tally.frontmatter_entries_removed += 1;
                    return;
                }
            }
        }

        if after_key.is_empty() && block.len() > 1 {
            if let Some(kept) = self.prune_list(home, block, tally) {
                if kept.is_empty() {
                    tally.frontmatter_keys_removed += 1;
                } else {
                    out.push_str(block[0]);
                    for line in kept {
                        out.push_str(&line);
                    }
                }
                return;
            }
        } else if block.len() == 1 && !after_key.is_empty() {
            if let Ok(Yaml::String(s)) = serde_yaml::from_str::<Yaml>(strip_comment(after_key)) {
                if self.whole_private_link(home, &s) {
                    tally.frontmatter_entries_removed += 1;
                    tally.frontmatter_keys_removed += 1;
                    return;
                }
            }
        }
        for line in block {
            out.push_str(&self.inline_keep_eol(home, line, tally));
        }
    }

    /// For a block-list key: the lines kept (empty means the key goes too), or
    /// `None` when the block is not a plain list.
    fn prune_list(
        &self,
        home: VaultKind,
        block: &[&str],
        tally: &mut Tally,
    ) -> Option<Vec<String>> {
        let indent = indent_of(block[1]);
        if !block[1][indent..].starts_with('-') {
            return None;
        }
        // Group lines into items: a line at the first item's indent that starts
        // with `-` opens one; anything else continues the previous item.
        let mut items: Vec<Vec<&str>> = Vec::new();
        for line in &block[1..] {
            let starts = indent_of(line) == indent && line[indent..].starts_with('-');
            if starts || items.is_empty() {
                items.push(vec![line]);
            } else if let Some(last) = items.last_mut() {
                last.push(line);
            }
        }
        let mut kept: Vec<String> = Vec::new();
        let mut removed = 0usize;
        let mut local = Tally::default();
        for item in &items {
            let first = split_eol(item[0]).0;
            let value = first[indent..].trim_start_matches('-').trim();
            let private = item.len() == 1
                && matches!(
                    serde_yaml::from_str::<Yaml>(strip_comment(value)),
                    Ok(Yaml::String(ref s)) if self.whole_private_link(home, s)
                );
            if private {
                removed += 1;
                continue;
            }
            for line in item {
                kept.push(self.inline_keep_eol(home, line, &mut local));
            }
        }
        tally.add(&local);
        tally.frontmatter_entries_removed += removed;
        Some(kept)
    }

    fn whole_private_link(&self, home: VaultKind, value: &str) -> bool {
        Wikilink::parse(value).is_some_and(|w| self.is_private(home, &w.target))
    }
}

// ------------------------------------------------------------------ helpers

fn split_eol(line: &str) -> (&str, &str) {
    if let Some(c) = line.strip_suffix("\r\n") {
        (c, "\r\n")
    } else if let Some(c) = line.strip_suffix('\n') {
        (c, "\n")
    } else {
        (line, "")
    }
}

/// `(target, shown)` of a wikilink's inner text. An escaped pipe (`\|`, used
/// inside tables) is a pipe.
fn split_link(inner: &str) -> (&str, Option<&str>) {
    match inner.split_once('|') {
        Some((t, s)) => {
            let t = t.strip_suffix('\\').unwrap_or(t);
            let s = s.trim();
            (t, (!s.is_empty()).then_some(s))
        }
        None => (inner, None),
    }
}

/// May the author's display text stand in for a private link? Not when it is
/// (or contains) a name that identifies the private page.
fn safe_label(shown: &str, entry: &Entry) -> bool {
    let low = shown.to_lowercase();
    let names = [&entry.title, &entry.id, &entry.slug]
        .into_iter()
        .chain(entry.aliases.iter());
    for n in names {
        let n = n.trim().to_lowercase();
        if !n.is_empty() && low.contains(&n) {
            return false;
        }
    }
    true
}

/// A fence opener: `(char, run length)` for a line of three or more backticks
/// or tildes.
fn opens_fence(content: &str) -> Option<(char, usize)> {
    let t = content.trim_start();
    let ch = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|c| *c == ch).count();
    (n >= 3).then_some((ch, n))
}

fn closes_fence(content: &str, ch: char, len: usize) -> bool {
    let t = content.trim();
    let n = t.chars().take_while(|c| *c == ch).count();
    n >= len && t.chars().all(|c| c == ch)
}

/// Byte offset of the next run of exactly `n` backticks in `s`.
fn find_run(s: &str, n: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'`' {
            let run = b[i..].iter().take_while(|c| **c == b'`').count();
            if run == n {
                return Some(i);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

fn is_fence_line(line: &str) -> bool {
    split_eol(line).0 == "---"
}

/// A top-level `key:` line: column 0, not a list item, comment or fence.
fn is_key_line(line: &str) -> bool {
    let c = split_eol(line).0;
    !c.is_empty()
        && !c.starts_with([' ', '\t', '#'])
        && !c.starts_with("- ")
        && c != "-"
        && c != "---"
        && c.contains(':')
}

/// Drop a trailing ` # comment` from a YAML scalar, outside quotes.
fn strip_comment(s: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    for (i, ch) in s.char_indices() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '#' && prev.is_whitespace() => return s[..i].trim_end(),
            None => {}
        }
        prev = ch;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault(kind: VaultKind, pages: &[(&str, &str)]) -> Vault {
        Vault {
            root: std::path::PathBuf::from("/v"),
            kind,
            pages: pages
                .iter()
                .map(|(id, text)| {
                    Page::parse(format!("/v/{id}.md"), format!("{id}.md"), *id, text).unwrap()
                })
                .collect(),
            journals: Vec::new(),
            skipped: Vec::new(),
        }
    }

    fn links() -> PrivateLinks {
        let k = vault(
            VaultKind::Knowledge,
            &[
                ("Open Page", "---\ntype: Class\npublic: true\n---\nx\n"),
                (
                    "Secret Thing",
                    "---\ntype: Class\npublic: false\naliases: [Hush, Secret Alias]\n---\nx\n",
                ),
                ("No Frontmatter", "just prose\n"),
                ("Düsseldorf Plan", "---\ntype: Class\n---\nx\n"),
            ],
        );
        let w = vault(
            VaultKind::Working,
            &[("Private Note", "---\ntype: Note\n---\nx\n")],
        );
        PrivateLinks::new(&[&k, &w])
    }

    fn body(text: &str) -> (String, Tally) {
        let mut t = Tally::default();
        let out = links().body(VaultKind::Knowledge, text, &mut t);
        (out, t)
    }

    fn fm(text: &str) -> (String, Tally) {
        let mut t = Tally::default();
        let out = links().frontmatter(VaultKind::Knowledge, text, &mut t);
        (out, t)
    }

    #[test]
    fn private_definition_is_withheld_whole_with_quoted_keys() {
        for key in ["defines-as", "'defines-as'", "\"defines-as\""] {
            let text = format!("---\n{key}:\n- '[[Open Page]]'\n- '[[Secret Thing]]'\n---\n");
            let mut tally = Tally::default();
            let got = links().frontmatter(VaultKind::Knowledge, &text, &mut tally);
            assert_eq!(
                got, "---\n---\n",
                "definition was weakened for {key}: {got}"
            );
        }
    }

    #[test]
    fn a_plain_link_to_a_private_page_becomes_the_marker() {
        let (out, t) = body("See [[Secret Thing]] now.\n");
        assert_eq!(out, "See (private link) now.\n");
        assert_eq!(t.body_links_rewritten, 1);
        assert_eq!(t.body_labels_kept, 0);
    }

    #[test]
    fn a_heading_link_becomes_the_marker() {
        assert_eq!(body("[[Secret Thing#Detail]]\n").0, "(private link)\n");
        assert_eq!(body("[[Secret Thing#^abc123]]\n").0, "(private link)\n");
    }

    #[test]
    fn a_safe_display_text_survives_and_an_unsafe_one_does_not() {
        assert_eq!(
            body("[[Secret Thing|a neutral alias]]\n").0,
            "a neutral alias\n"
        );
        assert_eq!(
            body("[[Secret Thing|secret thing]]\n").0,
            "(private link)\n"
        );
        assert_eq!(body("[[Secret Thing|Hush]]\n").0, "(private link)\n");
        assert_eq!(
            body("[[Secret Thing|the Secret Thing page]]\n").0,
            "(private link)\n"
        );
        assert_eq!(body("[[Secret Thing|]]\n").0, "(private link)\n");
        let (_, t) = body("[[Secret Thing|a neutral alias]]\n");
        assert_eq!(t.body_labels_kept, 1);
    }

    #[test]
    fn an_embed_of_a_private_page_is_removed() {
        let (out, t) = body("before ![[Secret Thing]] after\n");
        assert_eq!(out, "before  after\n");
        assert_eq!(t.embeds_removed, 1);
        assert_eq!(body("![[Secret Thing|300]]\n").0, "\n");
    }

    #[test]
    fn resolution_follows_the_builds_name_rules() {
        // case, alias, slug-less path forms and working-vault targets resolve
        assert_eq!(body("[[secret thing]]\n").0, "(private link)\n");
        assert_eq!(body("[[Hush]]\n").0, "(private link)\n");
        assert_eq!(body("[[Private Note]]\n").0, "(private link)\n");
        assert_eq!(body("[[No Frontmatter]]\n").0, "(private link)\n");
        assert_eq!(body("[[Secret Thing.md]]\n").0, "(private link)\n");
    }

    #[test]
    fn public_and_unresolved_targets_are_untouched() {
        let text = "[[Open Page]] [[Nothing Here]] [[Open Page|shown]] [[#Local]]\n";
        let (out, t) = body(text);
        assert_eq!(out, text);
        assert!(!t.changed());
    }

    #[test]
    fn code_is_authored_text_not_a_link() {
        let text = "```\n[[Secret Thing]]\n```\n`[[Secret Thing]]` and ``[[Secret Thing]]``\n~~~md\n[[Secret Thing]]\n~~~\n";
        let (out, t) = body(text);
        assert_eq!(out, text);
        assert!(!t.changed());
        let (out, _) = body("a `x` [[Secret Thing]] `y`\n");
        assert_eq!(out, "a `x` (private link) `y`\n");
        // An unclosed backtick is literal text, not a span.
        assert_eq!(body("`oops [[Secret Thing]]\n").0, "`oops (private link)\n");
        // A longer fence is not closed by a shorter run.
        let text = "````\n```\n[[Secret Thing]]\n````\n[[Secret Thing]]\n";
        assert_eq!(
            body(text).0,
            "````\n```\n[[Secret Thing]]\n````\n(private link)\n"
        );
    }

    #[test]
    fn crlf_and_missing_final_newline_are_preserved() {
        let (out, _) = body("one [[Secret Thing]]\r\ntwo\r\nthree [[Hush|ok]]");
        assert_eq!(out, "one (private link)\r\ntwo\r\nthree ok");
        let text = "```\r\n[[Secret Thing]]\r\n```\r\n";
        assert_eq!(body(text).0, text);
    }

    #[test]
    fn unicode_prose_is_byte_identical_around_a_rewrite() {
        let (out, _) =
            body("naïve – [[Secret Thing]] – 日本語 [[Düsseldorf Plan|Düsseldorf Plan]] ✓\n");
        assert_eq!(out, "naïve – (private link) – 日本語 (private link) ✓\n");
        let (out, _) = body("[[Secret Thing|日本語の別名]]\n");
        assert_eq!(out, "日本語の別名\n");
    }

    #[test]
    fn an_escaped_pipe_in_a_table_is_a_pipe() {
        assert_eq!(body("| [[Secret Thing\\|neutral]] |\n").0, "| neutral |\n");
    }

    #[test]
    fn a_working_page_resolves_into_the_knowledge_vault() {
        let mut t = Tally::default();
        let out = links().body(
            VaultKind::Working,
            "[[Secret Thing]] [[Open Page]]\n",
            &mut t,
        );
        assert_eq!(out, "(private link) [[Open Page]]\n");
    }

    #[test]
    fn a_held_back_folder_and_a_journal_are_non_public() {
        let mut k = vault(
            VaultKind::Knowledge,
            &[("Misc Page", "---\ntype: Class\npublic: true\n---\nx\n")],
        );
        k.pages[0].rel_path = std::path::PathBuf::from("misc/Misc Page.md");
        k.journals.push(
            Page::parse(
                "/v/journals/2026-10-01.md",
                "journals/2026-10-01.md",
                "2026-10-01",
                "x",
            )
            .unwrap(),
        );
        let l = PrivateLinks::new(&[&k]);
        let mut t = Tally::default();
        assert_eq!(
            l.body(
                VaultKind::Knowledge,
                "[[Misc Page]] [[2026-10-01]]\n",
                &mut t
            ),
            "(private link) (private link)\n"
        );
    }

    #[test]
    fn frontmatter_list_entries_are_removed_losslessly() {
        let text = "---\ntype: Class\nis-a:\n- '[[Secret Thing]]'\n- '[[Open Page]]'\nrelated-to:\n- '[[Hush]]'\ntitle: Keep Me\n---\n";
        let (out, t) = fm(text);
        assert_eq!(
            out,
            "---\ntype: Class\nis-a:\n- '[[Open Page]]'\ntitle: Keep Me\n---\n"
        );
        assert_eq!(t.frontmatter_entries_removed, 2);
        assert_eq!(t.frontmatter_keys_removed, 1);
    }

    #[test]
    fn a_scalar_link_to_a_private_page_removes_the_key() {
        let (out, t) = fm("---\nparent: '[[Secret Thing]]'\nother: '[[Open Page]]'\n---\n");
        assert_eq!(out, "---\nother: '[[Open Page]]'\n---\n");
        assert_eq!(t.frontmatter_keys_removed, 1);
    }

    #[test]
    fn an_untouched_frontmatter_is_byte_identical() {
        let text = "---\r\ntype: Class\r\n# a comment\r\nis-a:\r\n- '[[Open Page]]'\r\n- \"[[Nowhere]]\"\r\nnote: 'plain'\r\n---\r\n";
        let (out, t) = fm(text);
        assert_eq!(out, text);
        assert!(!t.changed());
    }

    #[test]
    fn crlf_frontmatter_is_edited_without_changing_other_line_endings() {
        let (out, _) =
            fm("---\r\nis-a:\r\n- '[[Secret Thing]]'\r\n- '[[Open Page]]'\r\nx: 1\r\n---\r\n");
        assert_eq!(out, "---\r\nis-a:\r\n- '[[Open Page]]'\r\nx: 1\r\n---\r\n");
    }

    #[test]
    fn a_link_inside_longer_frontmatter_text_is_rewritten_in_place() {
        let (out, t) = fm("---\ndescription: 'see [[Secret Thing]] and [[Open Page]]'\n---\n");
        assert_eq!(
            out,
            "---\ndescription: 'see (private link) and [[Open Page]]'\n---\n"
        );
        assert_eq!(t.frontmatter_inline_rewrites, 1);
    }

    #[test]
    fn nested_mappings_and_odd_quoting_do_not_confuse_the_editor() {
        let text = "---\ngenerated:\n  by: human:npub1x\n  at: 2026-10-11T00:00:00Z\nis-a:\n- \"[[Secret Thing]]\"\n- '[[Bob''s Page]]'\n---\n";
        let (out, _) = fm(text);
        assert_eq!(
            out,
            "---\ngenerated:\n  by: human:npub1x\n  at: 2026-10-11T00:00:00Z\nis-a:\n- '[[Bob''s Page]]'\n---\n"
        );
    }

    #[test]
    fn a_private_title_is_never_emitted() {
        let samples = [
            "x [[Secret Thing]] y\n",
            "x [[Secret Thing|Secret Thing]] y\n",
            "x [[secret thing#H|My Secret Thing note]] y\n",
            "![[Secret Thing]]\n",
            "x [[Hush]] y\n",
            "x [[Hush|HUSH]] y\n",
        ];
        for s in samples {
            let out = body(s).0.to_lowercase();
            for banned in ["secret thing", "hush", "secret alias"] {
                assert!(!out.contains(banned), "{s:?} -> {out:?}");
            }
        }
    }
}
