//! Fenced code block markers, the one rule every crate in the workspace uses.

/// The opening marker of a fenced code block: its character and run length.
///
/// A fence opens with three or more backticks or tildes after any
/// indentation and blockquote (`>`) prefix, so fences nested in lists and
/// callouts count. Following CommonMark, a backtick fence's info string may
/// not contain a backtick: a line such as ```` ```quoted prose``` ```` is an
/// inline code span, not a fence. A fence closes on a line holding only a run
/// of the same character at least as long.
///
/// ```
/// use ontomark_parse::Fence;
///
/// let fence = Fence::open("```rust").unwrap();
/// assert_eq!((fence.marker(), fence.len()), ('`', 3));
/// assert!(!fence.closes("~~~"));
/// assert!(fence.closes("  ````  "));
///
/// assert!(Fence::open("> ~~~~ mermaid").is_some()); // inside a callout
/// assert!(Fence::open("```inline code```").is_none());
/// assert!(Fence::open("``two``").is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    marker: char,
    len: usize,
}

impl Fence {
    /// The fence `line` opens, if it opens one.
    #[must_use]
    pub fn open(line: &str) -> Option<Self> {
        let lead = lead(line);
        let marker = lead.chars().next().filter(|c| matches!(c, '`' | '~'))?;
        let len = lead.chars().take_while(|&c| c == marker).count();
        let info = &lead[len..];
        (len >= 3 && !(marker == '`' && info.contains('`'))).then_some(Self { marker, len })
    }

    /// Whether `line` closes this fence.
    #[must_use]
    pub fn closes(self, line: &str) -> bool {
        let lead = lead(line).trim_end();
        lead.chars().take_while(|&c| c == self.marker).count() >= self.len
            && lead.chars().all(|c| c == self.marker)
    }

    /// The fence character: `` ` `` or `~`.
    #[must_use]
    pub fn marker(self) -> char {
        self.marker
    }

    /// The length of the opening run.
    #[must_use]
    #[allow(clippy::len_without_is_empty)] // a fence is never empty
    pub fn len(self) -> usize {
        self.len
    }
}

/// `line` without its indentation and blockquote prefix.
fn lead(line: &str) -> &str {
    line.trim_start_matches([' ', '\t', '>'])
        .trim_start_matches([' ', '\t'])
}

#[cfg(test)]
mod tests {
    use super::Fence;

    #[test]
    fn opens_on_three_or_more_markers() {
        assert!(Fence::open("``").is_none());
        assert_eq!(Fence::open("~~~~~").map(Fence::len), Some(5));
        assert!(
            Fence::open("\t\t- ```").is_none(),
            "a list bullet is not a prefix"
        );
        assert!(Fence::open("\t\t```json").is_some());
    }

    #[test]
    fn backtick_info_may_not_hold_a_backtick() {
        assert!(Fence::open("```With prose wrapped in a span.```").is_none());
        assert!(Fence::open("``` a ` b").is_none());
        assert!(
            Fence::open("~~~ a ` b").is_some(),
            "the rule is for backtick fences only"
        );
    }

    #[test]
    fn closes_only_on_a_bare_run_as_long_of_the_same_marker() {
        let fence = Fence::open("````").unwrap();
        assert!(!fence.closes("```"));
        assert!(!fence.closes("```` trailing"));
        assert!(!fence.closes("~~~~"));
        assert!(fence.closes("> `````\r"));
    }
}
