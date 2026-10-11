//! Guarded, lossless patches: the only write path.
//!
//! A [`Patch`] names the digest of the source it was planned against and a
//! list of operations. [`Document::apply`] refuses a patch whose digest does
//! not match, computes one byte-range edit per operation, and then *proves*
//! the result before returning it:
//!
//! 1. every byte outside the edited ranges is identical to the original;
//! 2. the result re-parses, its frontmatter is still patchable, and its keys
//!    are exactly the original keys with the requested additions and
//!    removals, in the original order;
//! 3. every key the patch did not name has byte-identical source text and an
//!    equal value;
//! 4. every key the patch set reads back as the requested value;
//! 5. a body-only patch leaves the frontmatter bytes identical, and no edit
//!    creates or destroys a frontmatter block.
//!
//! A proof failure is returned as [`PatchError::Verification`] and no output
//! is produced. Setting a key to the value it already has, or removing an
//! absent key, is a no-op, so applying the same patch twice changes nothing
//! the second time.

use std::fmt;

use crate::document::Document;
use crate::emit;
use crate::frontmatter::{BlockStyle, FrontmatterState};
use crate::span::{SourceDigest, Span};
use crate::value::Value;

/// Where [`PatchOp::Set`] puts a key that is not yet present. A key that is
/// present is always rewritten in place; existing keys are never reordered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "t", content = "key", rename_all = "lowercase")
)]
pub enum Placement {
    /// After the last entry (after any trailing comments), before the
    /// closing fence. The default.
    #[default]
    End,
    /// Immediately after the named entry.
    After(String),
    /// Before the first byte of the block's content.
    Start,
}

/// One operation of a [`Patch`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "op", rename_all = "snake_case"))]
pub enum PatchOp {
    /// Set a top-level frontmatter key. Creates the frontmatter block when
    /// the page has none.
    Set {
        /// The key.
        key: String,
        /// The new value.
        value: Value,
        /// Where a new key goes.
        #[cfg_attr(feature = "serde", serde(default))]
        placement: Placement,
    },
    /// Remove a top-level frontmatter key (no-op when absent).
    Remove {
        /// The key.
        key: String,
    },
    /// Replace a byte range of the body, guarded by its current text.
    ReplaceBody {
        /// The range, in document-source coordinates, inside the body.
        range: Span,
        /// The text the range must currently hold.
        expected: String,
        /// The replacement text.
        replacement: String,
    },
}

/// A guarded set of edits to one document.
///
/// ```
/// # use ontomark_parse::{Document, Patch, Value};
/// let doc = Document::parse("---\ntitle: A\n# keep me\nquality: 0.5\n---\nBody [[x]].\n");
/// let out = doc
///     .apply(&Patch::new(doc.digest()).set("quality", Value::Float("0.8".into())))
///     .unwrap();
/// assert_eq!(
///     out.document.source(),
///     "---\ntitle: A\n# keep me\nquality: 0.8\n---\nBody [[x]].\n"
/// );
/// assert_eq!(out.receipt.edits.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Patch {
    /// Digest of the source this patch was planned against.
    pub expected: SourceDigest,
    /// Operations, applied as one atomic edit.
    pub ops: Vec<PatchOp>,
}

impl Patch {
    /// An empty patch guarded by `expected`.
    #[must_use]
    pub fn new(expected: SourceDigest) -> Self {
        Self {
            expected,
            ops: Vec::new(),
        }
    }

    /// Add [`PatchOp::Set`] with [`Placement::End`].
    #[must_use]
    pub fn set(self, key: impl Into<String>, value: Value) -> Self {
        self.set_at(key, value, Placement::End)
    }

    /// Add [`PatchOp::Set`] with an explicit placement.
    #[must_use]
    pub fn set_at(mut self, key: impl Into<String>, value: Value, placement: Placement) -> Self {
        self.ops.push(PatchOp::Set {
            key: key.into(),
            value,
            placement,
        });
        self
    }

    /// Add [`PatchOp::Remove`].
    #[must_use]
    pub fn remove(mut self, key: impl Into<String>) -> Self {
        self.ops.push(PatchOp::Remove { key: key.into() });
        self
    }

    /// Add [`PatchOp::ReplaceBody`].
    #[must_use]
    pub fn replace_body(
        mut self,
        range: Span,
        expected: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.ops.push(PatchOp::ReplaceBody {
            range,
            expected: expected.into(),
            replacement: replacement.into(),
        });
        self
    }
}

/// One applied edit: the range it replaced in the old source and the range
/// its text occupies in the new source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Edit {
    /// Index of the operation in [`Patch::ops`].
    pub op: usize,
    /// Replaced range in the old source.
    pub before: Span,
    /// Range of the new text in the new source.
    pub after: Span,
}

/// Evidence of an applied patch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Receipt {
    /// Digest of the source the patch was applied to.
    pub before: SourceDigest,
    /// Digest of the result.
    pub after: SourceDigest,
    /// The edits, in source order. Empty when the patch was a no-op.
    pub edits: Vec<Edit>,
    /// Bytes outside the edits that were compared and found identical.
    pub preserved_bytes: usize,
}

/// A patched document and its receipt.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The new document.
    pub document: Document,
    /// What changed, and the proof summary.
    pub receipt: Receipt,
}

/// Why a patch was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatchError {
    /// The document is not the one the patch was planned against.
    #[error("source digest mismatch: patch expects {expected}, document is {actual}")]
    DigestMismatch {
        /// Digest the patch names.
        expected: SourceDigest,
        /// Digest of the document.
        actual: SourceDigest,
    },
    /// The frontmatter cannot be patched (unsupported construct or malformed).
    #[error("frontmatter cannot be patched: {0}")]
    FrontmatterLocked(String),
    /// A placement anchor key does not exist.
    #[error("placement anchor `{0}` is not a key of this page")]
    AnchorNotFound(String),
    /// A key is empty or contains a line break.
    #[error("invalid key {0:?}")]
    InvalidKey(String),
    /// Two operations touch the same key or overlapping ranges.
    #[error("operations {0} and {1} overlap")]
    Overlap(usize, usize),
    /// A body range is outside the body or not on character boundaries.
    #[error("body range {0} is outside the body or splits a character")]
    BadRange(Span),
    /// A body range no longer holds the expected text.
    #[error("body range {range} holds {found:?}, expected {expected:?}")]
    BodyGuard {
        /// The range.
        range: Span,
        /// What the patch expected.
        expected: String,
        /// What the document holds.
        found: String,
    },
    /// The result failed its post-condition proof; nothing was produced.
    #[error("patch verification failed: {0}")]
    Verification(String),
}

struct Planned {
    op: usize,
    /// Tie-break for insertions at the same byte: an insertion after an
    /// anchor (0) precedes other key edits (1), which precede body edits (2).
    /// This matches the key order the plan predicts.
    rank: u8,
    range: Span,
    text: String,
}

/// Expected key list after the patch, for verification.
struct KeyPlan {
    order: Vec<String>,
}

impl Document {
    /// Apply `patch`, returning the new document and a receipt, or refuse.
    ///
    /// # Errors
    /// See [`PatchError`]. On any error the original document is untouched
    /// and no partial result exists.
    pub fn apply(&self, patch: &Patch) -> Result<Applied, PatchError> {
        let actual = self.digest();
        if patch.expected != actual {
            return Err(PatchError::DigestMismatch {
                expected: patch.expected,
                actual,
            });
        }
        let src = self.source();
        let (planned, keys, set_values) = self.plan(patch)?;
        let mut planned = planned;
        planned.sort_by_key(|p| (p.range.start, p.range.end, p.rank, p.op));
        for w in planned.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let overlap = a.range.end > b.range.start
                || (a.range == b.range && !a.range.is_empty())
                || (a.range.is_empty()
                    && b.range.is_empty()
                    && a.range.start == b.range.start
                    && a.op == b.op);
            if overlap {
                return Err(PatchError::Overlap(a.op.min(b.op), a.op.max(b.op)));
            }
        }
        let mut out = String::with_capacity(src.len() + 64);
        let mut edits = Vec::with_capacity(planned.len());
        let mut cursor = 0;
        for p in &planned {
            out.push_str(&src[cursor..p.range.start]);
            let start = out.len();
            out.push_str(&p.text);
            edits.push(Edit {
                op: p.op,
                before: p.range,
                after: Span::new(start, out.len()),
            });
            cursor = p.range.end;
        }
        out.push_str(&src[cursor..]);
        let document = Document::parse(out);
        let preserved = self.verify(&document, &edits, &keys, &set_values, patch)?;
        let receipt = Receipt {
            before: actual,
            after: document.digest(),
            edits,
            preserved_bytes: preserved,
        };
        Ok(Applied { document, receipt })
    }

    #[allow(clippy::type_complexity)]
    fn plan(
        &self,
        patch: &Patch,
    ) -> Result<(Vec<Planned>, Option<KeyPlan>, Vec<(String, Value)>), PatchError> {
        let src = self.source();
        let touches_fm = patch
            .ops
            .iter()
            .any(|o| !matches!(o, PatchOp::ReplaceBody { .. }));
        let fm = self.frontmatter();
        if touches_fm {
            if let Some(f) = fm {
                match &f.state {
                    FrontmatterState::Parsed => {}
                    FrontmatterState::Unsupported(r) | FrontmatterState::Malformed(r) => {
                        return Err(PatchError::FrontmatterLocked(r.clone()));
                    }
                }
            }
        }
        let style = fm.map_or_else(
            || BlockStyle {
                crlf: src.contains("\r\n"),
                ..BlockStyle::default()
            },
            |f| f.style,
        );
        let mut order: Vec<String> = fm
            .map(|f| f.keys().map(str::to_owned).collect())
            .unwrap_or_default();
        let mut planned = Vec::new();
        let mut set_values = Vec::new();
        let mut named: Vec<(String, usize)> = Vec::new();
        // New keys for a page without frontmatter are gathered into one block.
        let mut new_block: Vec<(usize, String)> = Vec::new();
        let mut after: Vec<String> = Vec::new();
        let mut starts = 0usize;
        // Keys this patch removes: an insertion may not anchor on one, in
        // whichever order the two ops are written.
        let removed: Vec<(&str, usize)> = patch
            .ops
            .iter()
            .enumerate()
            .filter_map(|(i, o)| match o {
                PatchOp::Remove { key } => Some((key.as_str(), i)),
                _ => None,
            })
            .collect();
        for (i, op) in patch.ops.iter().enumerate() {
            match op {
                PatchOp::Set {
                    key,
                    value,
                    placement,
                } => {
                    check_key(key)?;
                    if let Some((_, j)) = named.iter().find(|(k, _)| k == key) {
                        return Err(PatchError::Overlap(*j, i));
                    }
                    named.push((key.clone(), i));
                    set_values.push((key.clone(), value.clone()));
                    let text = emit::entry(key, value, &style);
                    let Some(f) = fm else {
                        order.push(key.clone());
                        new_block.push((i, text));
                        continue;
                    };
                    if let Some(e) = f.get(key) {
                        if &e.value != value {
                            planned.push(Planned {
                                op: i,
                                rank: 1,
                                range: e.span,
                                text,
                            });
                        }
                        continue;
                    }
                    let at = match placement {
                        Placement::End => f.content.end,
                        Placement::Start => f.content.start,
                        Placement::After(anchor) => {
                            if let Some((_, j)) = removed.iter().find(|(k, _)| *k == anchor) {
                                return Err(PatchError::Overlap(i.min(*j), i.max(*j)));
                            }
                            let a = f
                                .get(anchor)
                                .ok_or_else(|| PatchError::AnchorNotFound(anchor.clone()))?;
                            // Earlier insertions after the same anchor stay first.
                            let prior = after.iter().filter(|x| *x == anchor).count();
                            let pos = order
                                .iter()
                                .position(|k| k == anchor)
                                .ok_or_else(|| PatchError::AnchorNotFound(anchor.clone()))?;
                            order.insert(pos + 1 + prior, key.clone());
                            after.push(anchor.clone());
                            planned.push(Planned {
                                op: i,
                                rank: 0,
                                range: Span::new(a.span.end, a.span.end),
                                text,
                            });
                            continue;
                        }
                    };
                    match placement {
                        Placement::Start => {
                            order.insert(starts, key.clone());
                            starts += 1;
                        }
                        _ => order.push(key.clone()),
                    }
                    planned.push(Planned {
                        op: i,
                        rank: 1,
                        range: Span::new(at, at),
                        text,
                    });
                }
                PatchOp::Remove { key } => {
                    check_key(key)?;
                    if let Some((_, j)) = named.iter().find(|(k, _)| k == key) {
                        return Err(PatchError::Overlap(*j, i));
                    }
                    named.push((key.clone(), i));
                    if let Some(e) = fm.and_then(|f| f.get(key)) {
                        order.retain(|k| k != key);
                        planned.push(Planned {
                            op: i,
                            rank: 1,
                            range: e.span,
                            text: String::new(),
                        });
                    }
                }
                PatchOp::ReplaceBody {
                    range,
                    expected,
                    replacement,
                } => {
                    let body = self.body_span();
                    if !body.contains(*range)
                        || !src.is_char_boundary(range.start)
                        || !src.is_char_boundary(range.end)
                    {
                        return Err(PatchError::BadRange(*range));
                    }
                    let found = range.slice(src);
                    if found != expected {
                        return Err(PatchError::BodyGuard {
                            range: *range,
                            expected: expected.clone(),
                            found: found.to_owned(),
                        });
                    }
                    if expected != replacement {
                        planned.push(Planned {
                            op: i,
                            rank: 2,
                            range: *range,
                            text: replacement.clone(),
                        });
                    }
                }
            }
        }
        if !new_block.is_empty() {
            let nl = if style.crlf { "\r\n" } else { "\n" };
            let mut text = format!("---{nl}");
            for (_, t) in &new_block {
                text.push_str(t);
            }
            text.push_str("---");
            text.push_str(nl);
            let at = self.body_span().start;
            planned.push(Planned {
                op: new_block[0].0,
                rank: 1,
                range: Span::new(at, at),
                text,
            });
        }
        let keys = touches_fm.then_some(KeyPlan { order });
        Ok((planned, keys, set_values))
    }

    fn verify(
        &self,
        new: &Document,
        edits: &[Edit],
        keys: &Option<KeyPlan>,
        set_values: &[(String, Value)],
        patch: &Patch,
    ) -> Result<usize, PatchError> {
        let fail = |m: String| Err(PatchError::Verification(m));
        let (old_src, new_src) = (self.source(), new.source());
        // 1. Bytes outside edits are identical.
        let mut preserved = 0;
        let (mut oc, mut nc) = (0usize, 0usize);
        for e in edits {
            let (o, n) = (&old_src[oc..e.before.start], &new_src[nc..e.after.start]);
            if o != n {
                return fail(format!("bytes before edit {} differ", e.op));
            }
            preserved += o.len();
            oc = e.before.end;
            nc = e.after.end;
        }
        if old_src[oc..] != new_src[nc..] {
            return fail("bytes after the last edit differ".into());
        }
        preserved += old_src.len() - oc;
        // 1b. Every edit stays on its own side of the frontmatter/body
        // boundary, and only key edits move that boundary. This is what stops
        // a body edit from writing frontmatter, with or without key ops.
        let new_body = new.body_span().start;
        let mut delta: isize = 0;
        for e in edits {
            let body_op = matches!(patch.ops[e.op], PatchOp::ReplaceBody { .. });
            if body_op {
                if e.after.start < new_body {
                    return fail(format!("body edit {} landed in the frontmatter", e.op));
                }
            } else {
                if e.after.end > new_body {
                    return fail(format!("key edit {} landed in the body", e.op));
                }
                delta += e.after.len() as isize - e.before.len() as isize;
            }
        }
        if self.body_span().start as isize + delta != new_body as isize {
            return fail("the body start moved by more than the key edits".into());
        }
        // 5. Frontmatter presence: only a Set on a page without one creates it.
        let creates =
            self.frontmatter().is_none() && keys.as_ref().is_some_and(|k| !k.order.is_empty());
        if new.frontmatter().is_some() != (self.frontmatter().is_some() || creates) {
            return fail("an edit created or destroyed a frontmatter block".into());
        }
        if keys.is_none() {
            // Body-only patch: the frontmatter bytes must be identical.
            let span = |d: &Document| {
                d.frontmatter()
                    .map(|f| Span::new(f.open_fence.start, f.close_fence.end))
            };
            if span(self).map(|s| s.slice(old_src)) != span(new).map(|s| s.slice(new_src)) {
                return fail("a body patch changed the frontmatter".into());
            }
            if self.body_span().start != new.body_span().start {
                return fail("a body patch moved the body start".into());
            }
            return Ok(preserved);
        }
        let Some(nf) = new.frontmatter() else {
            return Ok(preserved);
        };
        if nf.state != FrontmatterState::Parsed {
            return fail(format!("result frontmatter is {:?}", nf.state));
        }
        // 2. Key order.
        let plan = keys.as_ref().map(|k| &k.order);
        let got: Vec<&str> = nf.keys().collect();
        if plan.map(|o| o.iter().map(String::as_str).collect::<Vec<_>>()) != Some(got.clone()) {
            return fail(format!(
                "key order {got:?} does not match the plan {plan:?}"
            ));
        }
        // 3. Untouched keys: identical bytes and values.
        let named: Vec<&str> = patch
            .ops
            .iter()
            .filter_map(|o| match o {
                PatchOp::Set { key, .. } | PatchOp::Remove { key } => Some(key.as_str()),
                PatchOp::ReplaceBody { .. } => None,
            })
            .collect();
        if let Some(of) = self.frontmatter() {
            for e in &of.entries {
                if named.contains(&e.key.as_str()) {
                    continue;
                }
                let Some(ne) = nf.get(&e.key) else {
                    return fail(format!("untouched key `{}` disappeared", e.key));
                };
                if ne.raw(new_src) != e.raw(old_src) || ne.value != e.value {
                    return fail(format!("untouched key `{}` changed", e.key));
                }
            }
        }
        // 4. Set keys read back as requested.
        for (k, v) in set_values {
            if nf.value(k) != Some(v) {
                return fail(format!(
                    "key `{k}` reads back as {:?}, not {v:?}",
                    nf.value(k)
                ));
            }
        }
        Ok(preserved)
    }
}

fn check_key(key: &str) -> Result<(), PatchError> {
    if key.contains(['\n', '\r']) {
        return Err(PatchError::InvalidKey(key.to_owned()));
    }
    Ok(())
}

impl fmt::Display for Receipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} -> {}: {} edit(s), {} byte(s) proven unchanged",
            self.before,
            self.after,
            self.edits.len(),
            self.preserved_bytes
        )
    }
}
