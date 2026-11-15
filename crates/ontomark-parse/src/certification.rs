//! Versioned content bindings for human review (VisionFlow ADR-2024).
//! These hashes bind values, not authority: they are not signatures.
use crate::{Document, FrontmatterState, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Whether a recorded stamp is bound to this version of its reviewed content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// The stamp predates content bindings.
    Unbound,
    /// The digest matches the current reviewed content.
    Current,
    /// The content changed or the supplied binding is invalid.
    Stale,
}

/// Human review scopes and their freshness, independent of any host adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assessment {
    /// Historical whole-page protection, including stale and unbound review.
    pub whole_page: bool,
    /// Historically protected keys, sorted and deduplicated.
    pub keys: std::collections::BTreeSet<String>,
    /// Content-bound stamps matching the current source.
    pub current: usize,
    /// Stamps with changed content or malformed bindings/scopes.
    pub stale: usize,
    /// Historical stamps without a binding.
    pub unbound: usize,
}

/// Assess dated human stamps without granting authority to a digest.
/// Invalid scopes never claim current review. Valid historical scopes retain
/// ingest protection, even when their digest is malformed or stale.
#[must_use]
pub fn assess(doc: &Document, verified_key: &str) -> Assessment {
    let mut out = Assessment::default();
    let Some(Value::Seq(stamps)) = doc.value(verified_key) else {
        return out;
    };
    for stamp in stamps {
        let Value::Map(m) = stamp else {
            continue;
        };
        let human = m.get("by").and_then(Value::as_str).is_some_and(|s| {
            s.trim()
                .strip_prefix("human:")
                .is_some_and(|id| !id.trim().is_empty())
        });
        let dated = m
            .get("at")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty());
        if !human || !dated {
            continue;
        }
        // Preserve every identifiable legacy protection even if the scope is
        // malformed; malformed metadata must never unlock automated writes.
        match m.get("keys") {
            None | Some(Value::Null) => out.whole_page = true,
            Some(value) => out.keys.extend(
                value
                    .texts()
                    .into_iter()
                    .map(|k| k.trim().to_owned())
                    .filter(|k| !k.is_empty()),
            ),
        }
        let keys = match m.get("keys") {
            None | Some(Value::Null) => None,
            Some(Value::Seq(items))
                if !items.is_empty()
                    && items.iter().all(|v| {
                        v.as_str()
                            .is_some_and(|k| !k.trim().is_empty() && k != verified_key)
                    }) =>
            {
                Some(
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect::<Vec<_>>(),
                )
            }
            Some(_) => {
                out.stale += 1;
                continue;
            }
        };
        match &keys {
            None => out.whole_page = true,
            Some(keys) => out.keys.extend(keys.iter().cloned()),
        }
        let binding = m.get("content_digest");
        let digest = binding.and_then(Value::as_str);
        let status = if binding.is_some() && digest.is_none() {
            Freshness::Stale
        } else {
            freshness(doc, verified_key, keys.as_deref(), digest)
        };
        match status {
            Freshness::Current => out.current += 1,
            Freshness::Stale => out.stale += 1,
            Freshness::Unbound => out.unbound += 1,
        }
    }
    out
}

fn bytes(h: &mut Sha256, value: &[u8]) {
    h.update((value.len() as u64).to_be_bytes());
    h.update(value);
}
fn value(h: &mut Sha256, v: &Value) {
    match v {
        Value::Null => h.update([0]),
        Value::Bool(b) => h.update([1, u8::from(*b)]),
        Value::Int(i) => {
            h.update([2]);
            h.update(i.to_be_bytes());
        }
        Value::Float(s) => {
            h.update([3]);
            bytes(h, s.as_bytes());
        }
        Value::String(s) => {
            h.update([4]);
            bytes(h, s.as_bytes());
        }
        Value::Seq(vs) => {
            h.update([5]);
            h.update((vs.len() as u64).to_be_bytes());
            for v in vs {
                value(h, v);
            }
        }
        Value::Map(m) => {
            h.update([6]);
            h.update((m.len() as u64).to_be_bytes());
            for (k, v) in m.iter().collect::<BTreeMap<_, _>>() {
                bytes(h, k.as_bytes());
                value(h, v);
            }
        }
    }
}

/// Compute the binding for selected frontmatter keys, or the whole page.
/// Whole-page bindings exclude the verification list and include the exact body.
/// Map order is ignored; sequence order, types and missing values are significant.
/// Float spelling is preserved to avoid rounding distinct reviewed values.
/// Returns `None` for unsupported/malformed frontmatter or an invalid key scope.
#[must_use]
pub fn content_digest(
    doc: &Document,
    verified_key: &str,
    keys: Option<&[String]>,
) -> Option<String> {
    let mut h = Sha256::new();
    h.update(b"ontomark-review-v1\0");
    if doc
        .frontmatter()
        .is_some_and(|fm| fm.state != FrontmatterState::Parsed)
    {
        return None;
    }
    if let Some(keys) = keys {
        if keys.is_empty()
            || keys
                .iter()
                .any(|k| k.trim().is_empty() || k == verified_key)
        {
            return None;
        }
        h.update([1]);
        let ordered: std::collections::BTreeSet<_> = keys.iter().collect();
        for key in ordered {
            bytes(&mut h, key.as_bytes());
            match doc.value(key) {
                Some(v) => {
                    h.update([1]);
                    value(&mut h, v);
                }
                None => h.update([0]),
            }
        }
    } else {
        h.update([0]);
        if let Some(fm) = doc.frontmatter() {
            for (key, v) in fm
                .entries
                .iter()
                .filter(|e| e.key != verified_key)
                .map(|e| (&e.key, &e.value))
                .collect::<BTreeMap<_, _>>()
            {
                bytes(&mut h, key.as_bytes());
                value(&mut h, v);
            }
        }
        h.update([255]);
        bytes(&mut h, doc.body().as_bytes());
    }
    Some(format!(
        "sha256-v1:{}",
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

/// Compare an explicit stamp binding; absent binding remains historical/unbound.
#[must_use]
pub fn freshness(
    doc: &Document,
    verified_key: &str,
    keys: Option<&[String]>,
    digest: Option<&str>,
) -> Freshness {
    match digest {
        None => Freshness::Unbound,
        Some(expected) if content_digest(doc, verified_key, keys).as_deref() == Some(expected) => {
            Freshness::Current
        }
        Some(_) => Freshness::Stale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc(s: &str) -> Document {
        Document::parse(s)
    }
    #[test]
    fn reviewed_values_and_body_have_distinct_scope() {
        let a = doc("---\na: one\nb: two\n---\nbody");
        let b = doc("---\na: one\nb: changed\n---\nnew body");
        let keys = vec!["a".to_owned()];
        assert_eq!(
            content_digest(&a, "verified", Some(&keys)),
            content_digest(&b, "verified", Some(&keys))
        );
        assert_ne!(
            content_digest(&a, "verified", None),
            content_digest(&b, "verified", None)
        );
        let changed = doc("---\na: changed\n---\nbody");
        assert_ne!(
            content_digest(&a, "verified", Some(&keys)),
            content_digest(&changed, "verified", Some(&keys))
        );
    }
    #[test]
    fn order_types_and_history_are_explicit() {
        let a = doc("---\na: {x: 1, y: 2}\nb: [1, 2]\n---\nbody");
        let b = doc("---\nb: [1, 2]\na: {y: 2, x: 1}\nverified: []\n---\nbody");
        assert_eq!(
            content_digest(&a, "verified", None),
            content_digest(&b, "verified", None)
        );
        let digest = content_digest(&a, "verified", None).unwrap();
        assert_eq!(
            freshness(&a, "verified", None, Some(&digest)),
            Freshness::Current
        );
        assert_eq!(freshness(&a, "verified", None, None), Freshness::Unbound);
        assert_eq!(
            freshness(&a, "verified", None, Some("bad")),
            Freshness::Stale
        );
        assert_ne!(
            content_digest(&a, "verified", None),
            content_digest(
                &doc("---\na: {x: 1, y: 2}\nb: [2, 1]\n---\nbody"),
                "verified",
                None
            )
        );
    }
}
