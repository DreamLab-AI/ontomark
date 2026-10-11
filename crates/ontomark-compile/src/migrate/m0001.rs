//! Migration `m0001-obsidian-bodies`: Logseq outliner bodies to Obsidian
//! markdown.
//!
//! A wrapper over [`crate::check::bodies::convert`], so it agrees page for page
//! with `ontomark repair bodies`: a page that differs from its conversion only
//! in trailing whitespace is left exactly as its author saved it. It adds the
//! guards a migration needs on top of the repair command:
//!
//! * the frontmatter block (and any byte-order mark) must come through byte for
//!   byte, else the page is refused;
//! * the result must be a fixpoint of the conversion, else the page is refused
//!   (a second plan would not be empty);
//! * a file with uniform CRLF line endings is converted as LF and written back
//!   as CRLF; a file that mixes line endings is refused, because the original
//!   endings could not be restored line for line.
//!
//! ```
//! use ontomark_compile::migrate::{m0001::ObsidianBodies, Migration, Transform};
//!
//! let m = ObsidianBodies;
//! let page = "---\ntype: Note\n---\n- # Title\n\t- Point\n";
//! match m.transform("working/pages/X.md", page) {
//!     Transform::Changed { source, .. } => {
//!         assert_eq!(source, "---\ntype: Note\n---\n# Title\n\n- Point\n");
//!         // Idempotent: the result needs no further change.
//!         assert_eq!(m.transform("working/pages/X.md", &source), Transform::Unchanged);
//!     }
//!     other => panic!("{other:?}"),
//! }
//! ```

use super::{Kind, Migration, Transform};
use crate::check::bodies::{convert, same_but_trailing_whitespace};

/// The `m0001-obsidian-bodies` migration.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObsidianBodies;

impl Migration for ObsidianBodies {
    fn id(&self) -> &'static str {
        "m0001-obsidian-bodies"
    }

    fn version(&self) -> u32 {
        1
    }

    fn description(&self) -> &'static str {
        "Rewrite Logseq outliner page bodies as Obsidian markdown; frontmatter untouched"
    }

    fn kind(&self) -> Kind {
        Kind::Syntax
    }

    fn transform(&self, _page_rel_path: &str, source: &str) -> Transform {
        let crlf = source.contains('\r');
        let work = if crlf {
            let lf = source.replace("\r\n", "\n");
            if lf.contains('\r') {
                return Transform::Refused("bare CR line ending".to_owned());
            }
            if source.matches('\n').count() != source.matches("\r\n").count() {
                return Transform::Refused(
                    "mixed LF and CRLF line endings cannot be restored losslessly".to_owned(),
                );
            }
            lf
        } else {
            source.to_owned()
        };
        let after = convert(&work);
        if same_but_trailing_whitespace(&after, &work) {
            return Transform::Unchanged;
        }
        let head_len = work.len() - crate::input::frontmatter::split(&work).body.len();
        if !after.as_bytes().starts_with(&work.as_bytes()[..head_len]) {
            return Transform::Refused("conversion would alter the frontmatter block".to_owned());
        }
        if !same_but_trailing_whitespace(&convert(&after), &after) {
            return Transform::Refused("conversion is not a fixpoint".to_owned());
        }
        let summary = format!(
            "outline body converted ({} -> {} lines)",
            work.lines().count(),
            after.lines().count()
        );
        let source = if crlf {
            after.replace('\n', "\r\n")
        } else {
            after
        };
        Transform::Changed { source, summary }
    }
}
