//! The operations. Each is a method on [`Engine`](crate::Engine) returning a
//! [`Plan`](crate::Plan); none writes.

use ontomark_compile::input::page::{Page, VaultKind};
use ontomark_parse::{Document, SourceDigest, Value};

use crate::actor::{check_timestamp, Actor};
use crate::edit::{map_text, maps, text, Edit};
use crate::engine::Engine;
use crate::error::{code, Refusal, Result};

pub mod adopt;
pub mod create;
pub mod disputes;
pub mod patch;
pub mod supersede;
pub mod transition;

/// `true` when switching the contributor rule from "one entry per writer" to
/// "one entry per write" is wanted; see the report's open rulings.
pub const CONTRIBUTORS_ONE_PER_WRITER: bool = true;

/// Append the writer to `contributors`, once (ADR-VG-012 section 6).
///
/// With [`CONTRIBUTORS_ONE_PER_WRITER`] an entry whose `by` and `via` equal
/// the writer's is not added again; otherwise only an entry identical in `at`
/// as well is skipped.
pub(crate) fn append_contributor(edit: &mut Edit, actor: &Actor, now: &str) {
    let mut list: Vec<Value> = match edit.doc().value("contributors") {
        Some(Value::Seq(items)) => items.clone(),
        _ => Vec::new(),
    };
    let dup = maps(edit.doc(), "contributors").iter().any(|m| {
        map_text(m, "by").as_deref() == Some(actor.did.as_str())
            && map_text(m, "via") == actor.via
            && (CONTRIBUTORS_ONE_PER_WRITER || map_text(m, "at").as_deref() == Some(now))
    });
    if !dup {
        list.push(actor.stamp(now, true));
        edit.set("contributors", Value::Seq(list));
    }
}

/// The status of a page as text.
pub(crate) fn status_of(page: &Page) -> Option<String> {
    page.frontmatter.text("status")
}

/// `true` when `page` is a durable memory kind in the working vault.
pub(crate) fn is_memory(engine: &Engine, kind: VaultKind, page: &Page) -> bool {
    kind == VaultKind::Working
        && page
            .frontmatter
            .text("type")
            .is_some_and(|t| engine.vocab.working_type(&t).memory)
}

/// Statuses a memory page cannot leave (ADR-VG-013 section 1).
pub(crate) const TERMINAL: [&str; 3] = ["superseded", "invalidated", "archived"];

/// Check the optional base digest and the clock of a request.
pub(crate) fn check_base(page: &Page, expect: Option<&SourceDigest>) -> Result<()> {
    if let Some(d) = expect {
        let now = SourceDigest::of(&page.source);
        if &now != d {
            return Err(Refusal::new(
                code::STALE_BASE,
                format!("{} is {now}, the request was planned against {d}", page.id),
            ));
        }
    }
    Ok(())
}

/// A page's first stamp time, for the dispute staleness comparison.
pub(crate) fn content_clock(page: &Page) -> Option<String> {
    page.frontmatter.text("content-updated").or_else(|| {
        let doc = Document::parse(&page.source);
        match doc.value("generated") {
            Some(Value::Map(m)) => map_text(m, "at"),
            _ => text(&doc, "generated"),
        }
    })
}

/// Validate a timestamp, returning Unix seconds.
pub(crate) fn ts(what: &str, value: &str) -> Result<i64> {
    check_timestamp(what, value)
}

/// `true` when working type `ty` is a durable memory kind.
pub(crate) fn is_memory_type(engine: &Engine, ty: &str) -> bool {
    engine.vocab.working_type(ty).memory
}
