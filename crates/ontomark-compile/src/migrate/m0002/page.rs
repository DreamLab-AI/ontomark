//! Page rewrites of `m0002-vocabulary-v3`: pure functions of one page and the
//! [`Context`].

use std::collections::BTreeSet;

use indexmap::IndexMap;
use ontomark_parse::{Document, FrontmatterState, Patch, Placement, Value};

use super::{Context, Supersession, CANONICAL_PRINCIPAL};
use crate::input::principal::{did_body, npub_to_hex};
use crate::migrate::{sha256_hex, Transform};

/// The edits one page needs, collected, then applied as two guarded patches
/// (every set, then every removal), so the engine proves that nothing else in
/// the frontmatter or the body changed.
struct Edit<'d> {
    doc: &'d Document,
    sets: Vec<(String, Value, Placement)>,
    removes: Vec<String>,
    tags: Vec<&'static str>,
}

impl<'d> Edit<'d> {
    fn new(doc: &'d Document) -> Self {
        Self {
            doc,
            sets: Vec::new(),
            removes: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Set `key` to `value`, in place when it is there, unless it already is
    /// `value`. Only an effective set earns its change class.
    fn set(&mut self, tag: &'static str, key: &str, value: Value, at: Placement) {
        if self.doc.value(key) == Some(&value) {
            return;
        }
        self.sets.push((key.to_owned(), value, at));
        self.tag(tag);
    }

    fn remove(&mut self, tag: &'static str, key: &str) {
        if self.doc.value(key).is_some() {
            self.removes.push(key.to_owned());
            self.tag(tag);
        }
    }

    fn tag(&mut self, tag: &'static str) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    fn finish(self) -> Transform {
        let source = self.doc.source();
        let mut doc = self.doc.clone();
        for step in [true, false] {
            let mut patch = Patch::new(doc.digest());
            let mut any = false;
            if step {
                for (key, value, at) in &self.sets {
                    patch = patch.set_at(key.clone(), value.clone(), at.clone());
                    any = true;
                }
            } else {
                for key in &self.removes {
                    patch = patch.remove(key.clone());
                    any = true;
                }
            }
            if !any {
                continue;
            }
            match doc.apply(&patch) {
                Ok(applied) => doc = applied.document,
                Err(e) => return Transform::Refused(format!("the patch was refused: {e}")),
            }
        }
        if doc.source() == source {
            return Transform::Unchanged;
        }
        Transform::Changed {
            source: doc.source().to_owned(),
            summary: self.tags.join(", "),
        }
    }
}

fn refuse(reason: impl Into<String>) -> Transform {
    Transform::Refused(reason.into())
}

fn s(v: &str) -> Value {
    Value::string(v)
}

fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<IndexMap<_, _>>(),
    )
}

/// The principal a stamp's `by` becomes and the instrument it names, or why
/// neither can be said (ADR-VG-011 §1, ruling B8).
fn principal_of(ctx: &Context, by: &str) -> Result<(String, Option<String>), String> {
    let by = by.trim();
    if let Some(hex) = did_body(by) {
        return Ok(if ctx.human_keys.contains(hex) {
            (CANONICAL_PRINCIPAL.to_owned(), None)
        } else {
            (by.to_owned(), None)
        });
    }
    if let Some(rest) = by.strip_prefix("human:") {
        let rest = rest.trim();
        return match npub_to_hex(rest) {
            Ok(hex) if ctx.human_keys.contains(&hex) => Ok((CANONICAL_PRINCIPAL.to_owned(), None)),
            Ok(_) => Err(format!(
                "`{by}` is a key the roster does not list for a human"
            )),
            Err(_) if ctx.human_labels.contains(rest) => Ok((CANONICAL_PRINCIPAL.to_owned(), None)),
            Err(_) => Err(format!("`{by}` names no human in the roster")),
        };
    }
    if by.starts_with("agent:") || by.starts_with("process:") {
        return Ok((CANONICAL_PRINCIPAL.to_owned(), Some(by.to_owned())));
    }
    Err(format!("`by: {by}` is not an actor ontomark reads"))
}

/// A stamp in the principal and instrument form, or `None` when it already is.
fn restamp(ctx: &Context, stamp: &Value) -> Result<Option<Value>, String> {
    let Value::Map(m) = stamp else {
        return Err("a stamp that is not a mapping".to_owned());
    };
    let by = m
        .get("by")
        .and_then(Value::as_str)
        .ok_or_else(|| "a stamp with no readable `by`".to_owned())?;
    let (new_by, via) = principal_of(ctx, by)?;
    if new_by == by && via.is_none() {
        return Ok(None);
    }
    if via.is_some() && m.contains_key("via") {
        return Err(format!(
            "`by: {by}` is an instrument but the stamp already has a `via`"
        ));
    }
    let mut out: IndexMap<String, Value> = IndexMap::new();
    for (key, value) in m {
        if key == "by" {
            out.insert("by".into(), s(&new_by));
            if let Some(via) = &via {
                out.insert("via".into(), s(via));
            }
        } else {
            out.insert(key.clone(), value.clone());
        }
    }
    Ok(Some(Value::Map(out)))
}

fn stamps(ctx: &Context, doc: &Document, edit: &mut Edit<'_>) -> Result<(), String> {
    if let Some(stamp) = doc.value("generated") {
        if let Some(new) = restamp(ctx, stamp).map_err(|e| format!("`generated`: {e}"))? {
            edit.set("stamp", "generated", new, Placement::End);
        }
    }
    if let Some(Value::Seq(items)) = doc.value("verified") {
        let mut changed = false;
        let mut out = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            match restamp(ctx, item).map_err(|e| format!("`verified[{i}]`: {e}"))? {
                Some(new) => {
                    changed = true;
                    out.push(new);
                }
                None => out.push(item.clone()),
            }
        }
        if changed {
            edit.set("verified", "verified", Value::Seq(out), Placement::End);
        }
    }
    Ok(())
}

fn frontmatter_or_refuse(doc: &Document, what: &str) -> Result<(), Transform> {
    match doc.frontmatter() {
        None => Err(refuse(format!(
            "no frontmatter: {what}; add a header by hand if this page should be migrated"
        ))),
        Some(fm) if fm.state != FrontmatterState::Parsed => {
            Err(refuse("the frontmatter does not parse"))
        }
        Some(_) => Ok(()),
    }
}

/// A knowledge page: stamps, `verified`, merge annotations.
pub(crate) fn knowledge(ctx: &Context, rel: &str, source: &str) -> Transform {
    let doc = Document::parse(source);
    if doc.frontmatter().is_none() {
        // No frontmatter in knowledge/ is a private page (the vault contract).
        return Transform::Unchanged;
    }
    if let Err(t) = frontmatter_or_refuse(&doc, "") {
        return t;
    }
    let id = rel
        .strip_prefix("knowledge/")
        .unwrap_or(rel)
        .trim_end_matches(".md");
    let mut edit = Edit::new(&doc);
    if let Err(e) = stamps(ctx, &doc, &mut edit) {
        return refuse(e);
    }
    if let Some(entries) = ctx.supersessions.get(id) {
        if let Err(e) = supersedes(ctx, &doc, entries, &mut edit) {
            return refuse(e);
        }
    }
    edit.finish()
}

fn supersedes(
    ctx: &Context,
    doc: &Document,
    entries: &[Supersession],
    edit: &mut Edit<'_>,
) -> Result<(), String> {
    let mut list: Vec<Value> = match doc.value("supersedes") {
        Some(Value::Seq(items)) => items.clone(),
        Some(_) => return Err("`supersedes` is not a list".to_owned()),
        None => Vec::new(),
    };
    for e in entries {
        if !matches!(e.reason.as_str(), "merged" | "renamed") {
            return Err(format!(
                "{}: only merged or renamed losers are annotated here, not `{}`",
                e.resource, e.reason
            ));
        }
        if list
            .iter()
            .any(|v| matches!(v, Value::Map(m) if m.get("resource").and_then(Value::as_str) == Some(e.resource.as_str())))
        {
            continue;
        }
        let by = e.by.as_deref().unwrap_or(CANONICAL_PRINCIPAL);
        let (by, _) = principal_of(ctx, by).map_err(|m| format!("{}: {m}", e.resource))?;
        let mut entry = vec![
            ("resource", s(&e.resource)),
            ("reason", s(&e.reason)),
            ("at", s(&e.at)),
            ("by", s(&by)),
        ];
        if let Some(via) = &e.via {
            entry.push(("via", s(via)));
        }
        list.push(map(entry));
    }
    edit.set("supersedes", "supersedes", Value::Seq(list), Placement::End);
    let mut aliases: Vec<Value> = match doc.value("aliases") {
        Some(Value::Seq(items)) => items.clone(),
        Some(_) => return Err("`aliases` is not a list".to_owned()),
        None => Vec::new(),
    };
    let title = doc
        .value("title")
        .and_then(Value::as_str)
        .map(str::to_lowercase);
    let mut added = false;
    for e in entries {
        let Some(alias) = e.alias.as_deref() else {
            continue;
        };
        let known = aliases
            .iter()
            .filter_map(Value::as_str)
            .any(|a| a.eq_ignore_ascii_case(alias))
            || title.as_deref() == Some(alias.to_lowercase().as_str());
        if !known {
            aliases.push(s(alias));
            added = true;
        }
    }
    if added {
        let at = if doc.value("title").is_some() {
            Placement::After("title".to_owned())
        } else {
            Placement::End
        };
        edit.set("alias", "aliases", Value::Seq(aliases), at);
    }
    Ok(())
}

/// The recording URL and date from a transcript's `**Date**` and `**YouTube**`
/// lines.
pub(crate) fn transcript_facts(text: &str) -> Option<(String, String)> {
    let line = |label: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(&format!("- **{label}**:")))
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    Some((line("YouTube")?, line("Date")?))
}

/// The `sha256-v1:` pin of a page text, as ontomark pins it.
pub(crate) fn pin_of(text: &str) -> Option<String> {
    ontomark_parse::certification::content_digest(&Document::parse(text), "verified", None)
}

/// The wikilink targets in a page's body.
pub(crate) fn body_targets(text: &str) -> BTreeSet<String> {
    crate::input::page::Page::parse("x.md", "x.md", "x", text)
        .map(|p| {
            p.outbound_links()
                .into_iter()
                .map(|l| l.target.trim().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn page_resource(id: &str) -> String {
    format!(
        "urn:visionflow:page:{}",
        sha256_hex(format!("visionflow:page:working:{id}").as_bytes())
    )
}

fn ledger_id_of(resource: &str) -> Option<String> {
    let inner = resource
        .trim()
        .strip_prefix("[[")?
        .strip_suffix("]]")?
        .split('|')
        .next()?
        .trim();
    Some(inner.to_owned())
}

/// A working page.
pub(crate) fn working(ctx: &Context, rel: &str, source: &str) -> Transform {
    let doc = Document::parse(source);
    if let Err(t) = frontmatter_or_refuse(&doc, "private scratch has no header to migrate") {
        return t;
    }
    let id = rel
        .strip_prefix("working/")
        .unwrap_or(rel)
        .trim_end_matches(".md");
    let ty = doc
        .value("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if ty == "Draft Concept" {
        return refuse(
            "a Draft Concept retires into a Proposal, which needs `proposes` and located `evidence` that cannot be derived",
        );
    }
    if doc.value("generated").is_none() {
        return refuse(
            "no `generated` stamp: who generated this page is not derivable; add one by hand",
        );
    }
    let mut edit = Edit::new(&doc);
    if let Err(e) = stamps(ctx, &doc, &mut edit) {
        return refuse(e);
    }
    if let Err(e) = visibility(&doc, &mut edit) {
        return refuse(e);
    }
    if doc.value("page_resource").is_none() {
        edit.set(
            "page_resource",
            "page_resource",
            s(&page_resource(id)),
            Placement::End,
        );
    }
    let is_ledger = rel.starts_with(super::LEDGER_DIR) && ty == "Episode";
    let is_aggregate = doc.value("promotion-status").is_some() && !is_ledger;
    let result = if is_ledger {
        ledger(ctx, rel, &doc, &mut edit)
    } else if is_aggregate {
        aggregate(ctx, &doc, ty, &mut edit)
    } else if ty == "Episode" {
        Err("an Episode outside podcast-evidence/ with no `promotion-status`: neither a ledger nor a topic aggregate".to_owned())
    } else {
        if ty == "Note" && doc.value("resource").is_some() {
            edit.remove("class_keys", "resource");
            edit.remove("class_keys", "domain");
        }
        Ok(())
    };
    match result {
        Ok(()) => edit.finish(),
        Err(e) => refuse(e),
    }
}

fn visibility(doc: &Document, edit: &mut Edit<'_>) -> Result<(), String> {
    let public = match doc.value("public") {
        None => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => return Err("`public` is not a boolean".to_owned()),
    };
    match doc.value("visibility").map(Value::as_str) {
        Some(Some(existing)) => {
            if public.is_some_and(|p| p != (existing == "public")) {
                return Err(format!(
                    "`public: {}` contradicts `visibility: {existing}`",
                    public.unwrap_or_default()
                ));
            }
        }
        Some(None) => return Err("`visibility` is not a string".to_owned()),
        None => {
            let at = if public.is_some() {
                Placement::After("public".to_owned())
            } else {
                Placement::End
            };
            let value = if public == Some(true) {
                "public"
            } else {
                "shared"
            };
            edit.set("visibility", "visibility", s(value), at);
        }
    }
    edit.remove("visibility", "public");
    Ok(())
}

fn ledger(ctx: &Context, rel: &str, doc: &Document, edit: &mut Edit<'_>) -> Result<(), String> {
    if doc.value("status").and_then(Value::as_str) == Some("stable") {
        edit.set("episode_status", "status", s("approved"), Placement::End);
    }
    let slug = rel
        .rsplit('/')
        .next()
        .unwrap_or(rel)
        .trim_end_matches(".md");
    let (url, date) = ctx.transcripts.get(slug).ok_or_else(|| {
        format!("no transcripts/{slug}.md with a YouTube and a Date line: the recording is not derivable")
    })?;
    edit.set("ledger_origin", "episode-url", s(url), Placement::End);
    edit.set("ledger_origin", "episode-date", s(date), Placement::End);
    let origin = Value::Seq(vec![map(vec![
        ("id", s("origin")),
        ("rel", s("origin")),
        ("resource", s(url)),
    ])]);
    if let Some(existing) = doc.value("sources") {
        if existing != &origin {
            return Err("it already has `sources` other than the recording origin".to_owned());
        }
    }
    edit.set("ledger_origin", "sources", origin, Placement::End);
    Ok(())
}

fn aggregate(ctx: &Context, doc: &Document, ty: &str, edit: &mut Edit<'_>) -> Result<(), String> {
    if !matches!(ty, "Episode" | "Note" | "Brief") {
        return Err(format!(
            "a `{ty}` with a `promotion-status`: not a topic aggregate"
        ));
    }
    let topic = doc
        .value("topic")
        .and_then(Value::as_str)
        .or_else(|| doc.value("title").and_then(Value::as_str))
        .ok_or_else(|| "no `topic` or `title` to find its ledgers by".to_owned())?
        .to_owned();
    let derived: BTreeSet<String> = ctx.reverse.get(&topic).cloned().unwrap_or_default();

    // An explicit list of ledger links is kept in its order and only pinned;
    // a free-text source is replaced by the derived list.
    let explicit: Option<Vec<(String, IndexMap<String, Value>)>> = match doc.value("sources") {
        Some(Value::Seq(items)) if !items.is_empty() => items
            .iter()
            .map(|item| {
                let Value::Map(m) = item else { return None };
                let ledger = ledger_id_of(m.get("resource")?.as_str()?)?;
                ctx.ledgers
                    .contains_key(&ledger)
                    .then(|| (ledger, m.clone()))
            })
            .collect(),
        _ => None,
    };
    let items: Vec<(String, IndexMap<String, Value>)> = if let Some(list) = explicit {
        let listed: BTreeSet<String> = list.iter().map(|(l, _)| l.clone()).collect();
        if listed != derived {
            return Err(format!(
                "its {} listed ledger(s) differ from the {} ledger(s) whose bodies link [[{topic}]]",
                listed.len(),
                derived.len()
            ));
        }
        list
    } else {
        if let Some(Value::Int(n)) = doc.value("episodes") {
            if usize::try_from(*n).ok() != Some(derived.len()) {
                return Err(format!(
                    "{} ledger(s) link [[{topic}]] but it says `episodes: {n}`; merge or fix by hand (ADR-VG-010 §8)",
                    derived.len()
                ));
            }
        }
        if derived.is_empty() {
            return Err(format!(
                "no ledger links [[{topic}]], so its derived-from sources cannot be derived"
            ));
        }
        derived
            .iter()
            .map(|ledger| {
                let slug = ledger.rsplit('/').next().unwrap_or(ledger);
                let mut m = IndexMap::new();
                m.insert("id".to_owned(), s(slug));
                m.insert("resource".to_owned(), s(&format!("[[{ledger}]]")));
                (ledger.clone(), m)
            })
            .collect()
    };
    let sources: Vec<Value> = items
        .into_iter()
        .map(|(ledger, mut m)| {
            m.insert("rel".to_owned(), s("derived-from"));
            m.insert("locator".to_owned(), s(&ctx.ledgers[&ledger].pin));
            Value::Map(m)
        })
        .collect();
    if ty != "Brief" {
        edit.set("brief", "type", s("Brief"), Placement::End);
    }
    let after = if doc.value("status").is_some() {
        Placement::After("status".to_owned())
    } else {
        Placement::End
    };
    edit.set("brief", "scope", s("topic"), after);
    edit.set("brief", "sources", Value::Seq(sources), Placement::End);
    for key in ["source", "episodes", "assertions"] {
        edit.remove("count_keys", key);
    }
    Ok(())
}
