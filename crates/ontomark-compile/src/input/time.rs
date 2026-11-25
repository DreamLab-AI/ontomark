//! Authored time (ADR-VG-010 §2): `xsd:dateTime` in UTC at second precision,
//! and the one function that says which clock a supersession reason closes.
//!
//! ```
//! use ontomark_compile::input::time::{clocks_closed, parse_utc_seconds};
//!
//! assert!(parse_utc_seconds("2026-11-21T00:00:00Z").is_some());
//! assert!(parse_utc_seconds("2026-11-21").is_none()); // a date is not a dateTime
//! assert!(parse_utc_seconds("2026-11-21T00:00:00.5Z").is_none()); // second precision
//! assert!(parse_utc_seconds("2026-11-21T01:00:00+01:00").is_none()); // UTC only
//!
//! // World changed: valid time closes. Wrong or withdrawn: recorded time closes.
//! assert!(clocks_closed("world-changed").unwrap().valid);
//! assert!(clocks_closed("corrected").unwrap().recorded);
//! assert!(clocks_closed("merged").unwrap().is_none());
//! ```

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// The five reasons a `supersedes` entry may give (ADR-VG-010 §3).
pub const REASONS: [&str; 5] = [
    "world-changed",
    "corrected",
    "merged",
    "renamed",
    "withdrawn",
];

/// Seconds since the Unix epoch of a strict `YYYY-MM-DDTHH:MM:SSZ` value, or
/// `None` for anything else: a date, a fraction, an offset, a lower-case `z`,
/// or a value that is not a real instant.
#[must_use]
pub fn parse_utc_seconds(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    OffsetDateTime::parse(s, &Rfc3339)
        .ok()
        .map(OffsetDateTime::unix_timestamp)
}

/// Which of the two clocks a reason closes (ADR-VG-010 §3, ruling B1).
///
/// Valid time is when the fact held in the world; recorded time is when the
/// estate held the entry as true. They are never merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clocks {
    /// The entry stopped being true in the world (`valid-to`).
    pub valid: bool,
    /// The entry stopped being held as true (it was wrong or withdrawn).
    pub recorded: bool,
}

impl Clocks {
    /// `true` when neither clock closes: identity curation, not a retraction.
    #[must_use]
    pub fn is_none(self) -> bool {
        !self.valid && !self.recorded
    }
}

/// The clocks a supersession `reason` closes, or `None` for a word that is
/// not a reason.
///
/// * `world-changed` closes valid time.
/// * `corrected` and `withdrawn` close recorded time: every withdrawal closes
///   at least one clock, as memory-ontology requires.
/// * `merged` and `renamed` are identity curation and close neither.
///
/// Every rule that cares which clock a reason closes calls this function.
#[must_use]
pub fn clocks_closed(reason: &str) -> Option<Clocks> {
    Some(match reason {
        "world-changed" => Clocks {
            valid: true,
            recorded: false,
        },
        "corrected" | "withdrawn" => Clocks {
            valid: false,
            recorded: true,
        },
        "merged" | "renamed" => Clocks::default(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_strict_utc_second_precision_datetime_parses() {
        assert_eq!(parse_utc_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_utc_seconds("2026-11-21T10:00:00Z"),
            Some(1_795_255_200)
        );
        for bad in [
            "2026-11-21",
            "2026-11-21T10:00Z",
            "2026-11-21T10:00:00",
            "2026-11-21T10:00:00z",
            "2026-11-21T10:00:00.000Z",
            "2026-11-21T10:00:00+00:00",
            "2026-11-21 10:00:00Z",
            "2026-13-21T10:00:00Z",
            "2026-02-30T10:00:00Z",
            "2026-11-21T25:00:00Z",
            "",
        ] {
            assert_eq!(parse_utc_seconds(bad), None, "{bad}");
        }
    }

    #[test]
    fn each_reason_closes_the_clock_the_ruling_says() {
        let c = |r| clocks_closed(r).unwrap();
        assert_eq!(
            (c("world-changed").valid, c("world-changed").recorded),
            (true, false)
        );
        assert_eq!(
            (c("corrected").valid, c("corrected").recorded),
            (false, true)
        );
        assert_eq!(
            (c("withdrawn").valid, c("withdrawn").recorded),
            (false, true)
        );
        assert!(c("merged").is_none() && c("renamed").is_none());
        assert!(clocks_closed("deleted").is_none());
        for reason in REASONS {
            assert!(clocks_closed(reason).is_some(), "{reason}");
        }
    }
}
