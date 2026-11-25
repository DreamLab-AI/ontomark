//! The host services the core deliberately lacks: the wall clock and OS
//! randomness. Off for wasm hosts, which pass `now` and entropy in.

use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

/// The current instant as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// ```
/// let now = ontomark_write::system::now();
/// assert!(ontomark_write::check_timestamp("now", &now).is_ok());
/// ```
#[must_use]
pub fn now() -> String {
    let t = OffsetDateTime::now_utc()
        .to_offset(UtcOffset::UTC)
        .replace_nanosecond(0)
        .unwrap_or_else(|_| OffsetDateTime::now_utc());
    t.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

/// 32 bytes from the operating system's random source, for a page identity.
///
/// # Errors
/// When the OS cannot supply randomness.
pub fn entropy() -> Result<[u8; 32], getrandom::Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes)
}
