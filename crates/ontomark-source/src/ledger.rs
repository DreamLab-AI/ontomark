//! The change ledger and classification.

use crate::error::{Result, SourceError};
use crate::{Fetched, SourceItem};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Current ledger file format number.
const FORMAT: u32 = 1;

/// What was recorded the last time an item was processed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Version (SHA-256 of the rendered text) at that time.
    pub version: String,
    /// The parser profile in force at that time (any host-chosen string that
    /// changes when rendering or chunking rules change).
    pub parser_profile: String,
    /// The change marker seen at that time.
    pub change_marker: String,
}

/// Which items have been processed, at which version and parser profile.
///
/// Serialises as JSON, keyed by item id in sorted order, so a ledger diffs
/// cleanly. Items are keyed by [`SourceItem::id`]; use one ledger per source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    format: u32,
    /// Entries by item id.
    items: BTreeMap<String, LedgerEntry>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

impl Ledger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self {
            format: FORMAT,
            items: BTreeMap::new(),
        }
    }

    /// The entry for an item id, if any.
    pub fn get(&self, item_id: &str) -> Option<&LedgerEntry> {
        self.items.get(item_id)
    }

    /// Number of items recorded.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing is recorded.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Entries in item-id order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &LedgerEntry)> {
        self.items.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Record that `item`, read as `fetched`, was processed under `parser_profile`.
    pub fn record(&mut self, item: &SourceItem, fetched: &Fetched, parser_profile: &str) {
        self.items.insert(
            item.id.clone(),
            LedgerEntry {
                version: fetched.version.clone(),
                parser_profile: parser_profile.to_string(),
                change_marker: item.change_marker.clone(),
            },
        );
    }

    /// Drop an item (for example once a [`Change::Removed`] is handled).
    /// Returns the entry that was removed.
    pub fn forget(&mut self, item_id: &str) -> Option<LedgerEntry> {
        self.items.remove(item_id)
    }

    /// The cheap pre-check: true when `item` can be skipped without fetching,
    /// because it is recorded with the same parser profile and the same
    /// *non-empty* change marker. An empty marker (a URL without `ETag` or
    /// `Last-Modified`) is never skippable.
    pub fn can_skip(&self, item: &SourceItem, parser_profile: &str) -> bool {
        !item.change_marker.is_empty()
            && self.items.get(&item.id).is_some_and(|e| {
                e.parser_profile == parser_profile && e.change_marker == item.change_marker
            })
    }

    /// Read a ledger from `path`. A missing file is an error; see [`Ledger::read_or_new`].
    pub fn read(path: &Path) -> Result<Self> {
        let err = |message: String| SourceError::Ledger {
            locator: path.display().to_string(),
            message,
        };
        let bytes = std::fs::read(path).map_err(|e| err(e.to_string()))?;
        let ledger: Ledger = serde_json::from_slice(&bytes).map_err(|e| err(e.to_string()))?;
        if ledger.format != FORMAT {
            return Err(err(format!("unsupported ledger format {}", ledger.format)));
        }
        Ok(ledger)
    }

    /// Read a ledger from `path`, or return an empty one if the file does not exist.
    pub fn read_or_new(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::read(path)
        } else {
            Ok(Self::new())
        }
    }

    /// Write the ledger to `path` as pretty JSON, via a temporary sibling file
    /// and a rename so an interrupted write never leaves a truncated ledger.
    pub fn write(&self, path: &Path) -> Result<()> {
        let err = |message: String| SourceError::Ledger {
            locator: path.display().to_string(),
            message,
        };
        let mut json = serde_json::to_string_pretty(self).map_err(|e| err(e.to_string()))?;
        json.push('\n');
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = std::path::PathBuf::from(tmp);
        std::fs::write(&tmp, json).map_err(|e| err(e.to_string()))?;
        std::fs::rename(&tmp, path).map_err(|e| err(e.to_string()))
    }
}

/// How an item differs from the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Not in the ledger.
    New {
        /// The item id.
        item_id: String,
    },
    /// The rendered text differs (different version): re-read.
    ContentChanged {
        /// The item id.
        item_id: String,
    },
    /// Same text, but the parser profile differs: re-process. Also reported,
    /// without a fetch, when the profile differs and content was not checked.
    ParserChanged {
        /// The item id.
        item_id: String,
    },
    /// Same text and profile, but the change marker differs (a touch or
    /// rename of metadata): nothing to re-read; update the ledger marker.
    MarkerOnly {
        /// The item id.
        item_id: String,
    },
    /// Same text, profile and marker.
    Unchanged {
        /// The item id.
        item_id: String,
    },
    /// In the ledger but no longer offered by the source.
    Removed {
        /// The item id.
        item_id: String,
    },
    /// Recorded, profile unchanged, but the marker differs or is empty and no
    /// fetch was supplied: fetch the item and classify again. Never a guess.
    NeedsFetch {
        /// The item id.
        item_id: String,
    },
}

impl Change {
    /// The item id this change concerns.
    pub fn item_id(&self) -> &str {
        match self {
            Change::New { item_id }
            | Change::ContentChanged { item_id }
            | Change::ParserChanged { item_id }
            | Change::MarkerOnly { item_id }
            | Change::Unchanged { item_id }
            | Change::Removed { item_id }
            | Change::NeedsFetch { item_id } => item_id,
        }
    }

    /// True for the changes that call for processing the item again:
    /// `New`, `ContentChanged` and `ParserChanged`.
    pub fn needs_processing(&self) -> bool {
        matches!(
            self,
            Change::New { .. } | Change::ContentChanged { .. } | Change::ParserChanged { .. }
        )
    }
}

/// Classify a listing against the ledger.
///
/// `observed` pairs each listed item with its fetched text, or `None` when it
/// was not fetched (because [`Ledger::can_skip`] said so, or to ask what is
/// decidable without a read). One result per observed item, in input order,
/// then a [`Change::Removed`] for each ledger id not observed, in id order.
///
/// With a fetch: version differs is `ContentChanged`; else profile differs is
/// `ParserChanged`; else marker differs is `MarkerOnly`; else `Unchanged`.
/// Without a fetch: not in the ledger is `New`; profile differs is
/// `ParserChanged`; a matching non-empty marker is `Unchanged`; anything else
/// is `NeedsFetch`.
pub fn classify(
    ledger: &Ledger,
    observed: &[(SourceItem, Option<Fetched>)],
    parser_profile: &str,
) -> Vec<Change> {
    let mut out = Vec::with_capacity(observed.len());
    let mut seen = BTreeSet::new();
    for (item, fetched) in observed {
        seen.insert(item.id.as_str());
        let item_id = item.id.clone();
        let change = match (ledger.get(&item.id), fetched) {
            (None, _) => Change::New { item_id },
            (Some(e), Some(f)) => {
                if e.version != f.version {
                    Change::ContentChanged { item_id }
                } else if e.parser_profile != parser_profile {
                    Change::ParserChanged { item_id }
                } else if e.change_marker != item.change_marker {
                    Change::MarkerOnly { item_id }
                } else {
                    Change::Unchanged { item_id }
                }
            }
            (Some(e), None) => {
                if e.parser_profile != parser_profile {
                    Change::ParserChanged { item_id }
                } else if ledger.can_skip(item, parser_profile) {
                    Change::Unchanged { item_id }
                } else {
                    Change::NeedsFetch { item_id }
                }
            }
        };
        out.push(change);
    }
    for (id, _) in ledger.iter() {
        if !seen.contains(id) {
            out.push(Change::Removed {
                item_id: id.to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, marker: &str) -> SourceItem {
        SourceItem {
            id: id.into(),
            locator: id.into(),
            change_marker: marker.into(),
            size: None,
        }
    }
    fn fetched(id: &str, text: &str) -> Fetched {
        Fetched::new(id, "text/plain", text)
    }
    fn ledger() -> Ledger {
        let mut l = Ledger::new();
        l.record(&item("a", "m1"), &fetched("a", "one"), "p1");
        l.record(&item("gone", "m"), &fetched("gone", "x"), "p1");
        l
    }
    fn one(l: &Ledger, i: SourceItem, f: Option<Fetched>, p: &str) -> Change {
        classify(l, &[(i, f)], p).into_iter().next().unwrap()
    }

    #[test]
    fn every_class_with_a_fetch() {
        let l = ledger();
        let id = |s: &str| s.to_string();
        assert_eq!(
            one(&l, item("n", "m"), Some(fetched("n", "t")), "p1"),
            Change::New { item_id: id("n") }
        );
        assert_eq!(
            one(&l, item("a", "m1"), Some(fetched("a", "two")), "p1"),
            Change::ContentChanged { item_id: id("a") }
        );
        assert_eq!(
            one(&l, item("a", "m1"), Some(fetched("a", "one")), "p2"),
            Change::ParserChanged { item_id: id("a") }
        );
        assert_eq!(
            one(&l, item("a", "m2"), Some(fetched("a", "one")), "p1"),
            Change::MarkerOnly { item_id: id("a") }
        );
        assert_eq!(
            one(&l, item("a", "m1"), Some(fetched("a", "one")), "p1"),
            Change::Unchanged { item_id: id("a") }
        );
    }

    #[test]
    fn removed_follow_observed_in_id_order() {
        let mut l = ledger();
        l.record(&item("b", "m"), &fetched("b", "x"), "p1");
        let out = classify(&l, &[(item("a", "m1"), None)], "p1");
        let ids: Vec<_> = out.iter().map(|c| c.item_id()).collect();
        assert_eq!(ids, ["a", "b", "gone"]);
        assert!(matches!(out[1], Change::Removed { .. }));
    }

    #[test]
    fn precheck_skips_only_with_a_real_marker_and_same_profile() {
        let l = ledger();
        assert!(l.can_skip(&item("a", "m1"), "p1"));
        assert!(!l.can_skip(&item("a", "m1"), "p2"));
        assert!(!l.can_skip(&item("a", "m9"), "p1"));
        assert!(!l.can_skip(&item("zzz", "m1"), "p1"));
        let mut l2 = Ledger::new();
        l2.record(&item("u", ""), &fetched("u", "t"), "p1");
        assert!(!l2.can_skip(&item("u", ""), "p1"));
    }

    #[test]
    fn without_a_fetch_nothing_is_guessed() {
        let l = ledger();
        let id = |s: &str| s.to_string();
        assert_eq!(
            one(&l, item("a", "m1"), None, "p1"),
            Change::Unchanged { item_id: id("a") }
        );
        assert_eq!(
            one(&l, item("a", "m1"), None, "p2"),
            Change::ParserChanged { item_id: id("a") }
        );
        assert_eq!(
            one(&l, item("a", "m2"), None, "p1"),
            Change::NeedsFetch { item_id: id("a") }
        );
        assert_eq!(
            one(&l, item("n", "m"), None, "p1"),
            Change::New { item_id: id("n") }
        );
    }

    #[test]
    fn rename_free_touch_is_not_a_reread() {
        // Same text under a new marker is MarkerOnly, which needs no processing.
        let l = ledger();
        let c = one(&l, item("a", "later"), Some(fetched("a", "one")), "p1");
        assert!(!c.needs_processing());
    }

    #[test]
    fn json_roundtrip_via_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ledger.json");
        assert!(Ledger::read(&p).is_err());
        assert!(Ledger::read_or_new(&p).unwrap().is_empty());
        let l = ledger();
        l.write(&p).unwrap();
        assert_eq!(Ledger::read(&p).unwrap(), l);
        assert!(!d.path().join("ledger.json.tmp").exists());
        std::fs::write(&p, "{\"format\":9,\"items\":{}}").unwrap();
        assert!(Ledger::read(&p).is_err());
        std::fs::write(&p, "nope").unwrap();
        assert!(Ledger::read(&p).is_err());
    }
}
