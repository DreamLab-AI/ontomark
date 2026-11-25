//! The operations. Each is a method on [`Engine`](crate::Engine) returning a
//! [`Plan`](crate::Plan); none writes.

use ontomark_compile::input::page::{Page, VaultKind};
use ontomark_parse::{Document, SourceDigest, Value};

use crate::actor::{check_timestamp, Actor};
use crate::edit::{map_text, text, Edit};
use crate::engine::Engine;
use crate::error::{code, Refusal, Result};

pub mod adopt;
pub mod create;
pub mod disputes;
pub mod patch;
pub mod supersede;
pub mod transition;

/// Append one `contributors` entry for this write (ADR-VG-012 section 6:
/// "every change, oldest first"; ADR-VG-013 section 7: `contributors` is the
/// audit trail). One entry per WRITE, never per writer: with a single
/// principal for the whole estate, a per-writer rule would record nothing after
/// the first write. A write is a stale-guarded commit, so a replay cannot add
/// the entry twice.
pub(crate) fn append_contributor(edit: &mut Edit, actor: &Actor, now: &str) {
    let mut list: Vec<Value> = match edit.doc().value("contributors") {
        Some(Value::Seq(items)) => items.clone(),
        _ => Vec::new(),
    };
    list.push(actor.stamp(now, true));
    edit.set("contributors", Value::Seq(list));
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
