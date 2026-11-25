//! The lossless edit builder every operation goes through.
//!
//! An [`Edit`] collects [`PatchOp`]s against one page's exact text and applies
//! them as one guarded `ontomark_parse` patch. Nothing is re-serialised: keys
//! the edit does not name, their order, their comments, unknown keys and the
//! body keep their bytes, and the parse crate proves it before returning.

use indexmap::IndexMap;
use ontomark_parse::{Document, Patch, PatchOp, Placement, Receipt, Value};

use crate::error::{code, Refusal, Result};

/// A pending patch over one page.
#[derive(Clone)]
pub(crate) struct Edit {
    doc: Document,
    ops: Vec<PatchOp>,
}

impl Edit {
    /// Start an edit over `text`.
    pub(crate) fn new(text: &str) -> Self {
        Self {
            doc: Document::parse(text),
            ops: Vec::new(),
        }
    }

    /// The page as it is now (pending operations are not visible).
    pub(crate) fn doc(&self) -> &Document {
        &self.doc
    }

    /// Set `key`, in place when present and at the end otherwise.
    pub(crate) fn set(&mut self, key: &str, value: Value) {
        self.ops.push(PatchOp::Set {
            key: key.to_owned(),
            value,
            placement: Placement::End,
        });
    }

    /// Remove `key` (a no-op when absent).
    pub(crate) fn remove(&mut self, key: &str) {
        self.ops.push(PatchOp::Remove {
            key: key.to_owned(),
        });
    }

    /// Replace a body range, guarded by its current text.
    pub(crate) fn replace_body(&mut self, op: PatchOp) {
        self.ops.push(op);
    }

    /// Apply every pending operation as one patch. Returns the new text and the
    /// parse crate's receipt.
    pub(crate) fn finish(self) -> Result<(String, Receipt)> {
        let mut patch = Patch::new(self.doc.digest());
        patch.ops = self.ops;
        let applied = self.doc.apply(&patch).map_err(|e| {
            Refusal::new(
                code::UNPATCHABLE,
                format!("the lossless patch refused: {e}"),
            )
        })?;
        Ok((applied.document.into_source(), applied.receipt))
    }
}

/// The scalar text of `key`.
pub(crate) fn text(doc: &Document, key: &str) -> Option<String> {
    doc.value(key).and_then(Value::scalar_text)
}

/// `key` as a list of mappings.
pub(crate) fn maps(doc: &Document, key: &str) -> Vec<IndexMap<String, Value>> {
    match doc.value(key) {
        Some(Value::Seq(items)) => items.iter().filter_map(|v| v.as_map().cloned()).collect(),
        _ => Vec::new(),
    }
}

/// `key` of a mapping as text.
pub(crate) fn map_text(m: &IndexMap<String, Value>, key: &str) -> Option<String> {
    m.get(key).and_then(Value::scalar_text)
}

/// A mapping value from `(key, value)` pairs.
pub(crate) fn map<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    Value::Map(
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<IndexMap<_, _>>(),
    )
}
