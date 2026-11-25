//! Which frontmatter keys the engine owns, and which count as content.
//!
//! ADR-VG-013 section 7: **content** is the body and every key except the
//! lifecycle set. A write that changes content moves `content-updated`; a
//! review, status, dispute, visibility or bookkeeping write does not.

/// Keys that are lifecycle or bookkeeping, not content (ADR-VG-013 section 7).
pub const LIFECYCLE_KEYS: [&str; 10] = [
    "status",
    "status-reason",
    "disputes",
    "last-reviewed",
    "superseded-by",
    "supersedes",
    "valid-to",
    "visibility",
    "contributors",
    "content-updated",
];

/// The two review clocks. A value a client sends for either is ignored.
pub const CLOCK_KEYS: [&str; 2] = ["last-reviewed", "content-updated"];

/// Keys no patch may change on any page once present: identity and origin.
pub const IMMUTABLE_KEYS: [&str; 2] = ["page_resource", "generated"];

/// Keys only a lifecycle operation writes on a durable memory page.
pub const MEMORY_OWNED_KEYS: [&str; 8] = [
    "status",
    "status-reason",
    "disputes",
    "superseded-by",
    "supersedes",
    "valid-to",
    "contributors",
    "last-reviewed",
];

/// `true` when changing `key` is a content change.
#[must_use]
pub fn is_content_key(key: &str) -> bool {
    !LIFECYCLE_KEYS.contains(&key)
}
