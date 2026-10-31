//! Lossless Markdown document model for Obsidian pages.
//!
//! `ontomark-parse` owns the Markdown format in the ontomark workspace. A
//! [`Document`] keeps the page's exact bytes and indexes them:
//!
//! * the YAML frontmatter block, split into top-level [`Entry`] spans with
//!   typed [`Value`]s, comments and blank lines kept as gaps;
//! * the body as opaque text, with an index of wikilinks, embeds, footnote
//!   references and definitions, and the code regions where those are not
//!   recognised ([`BodyIndex`]).
//!
//! Nothing is ever re-rendered from the typed view. The only write path is a
//! [`Patch`]: guarded by the source digest it was planned against, it rewrites
//! exactly the keys or body ranges it names and proves that every other byte
//! is unchanged before it returns ([`Receipt`]).
//!
//! ```
//! use ontomark_parse::{Document, Patch, Value};
//!
//! let src = "---\ntype: Class\n# reviewed\naliases:\n- KG\n---\nA [[Graph]].\n";
//! let doc = Document::parse(src);
//! assert_eq!(doc.source(), src); // lossless
//!
//! let patch = Patch::new(doc.digest()).set("aliases", Value::strings(["KG", "Knowledge graph"]));
//! let out = doc.apply(&patch).unwrap();
//! assert_eq!(
//!     out.document.source(),
//!     "---\ntype: Class\n# reviewed\naliases:\n- KG\n- Knowledge graph\n---\nA [[Graph]].\n"
//! );
//!
//! // A patch planned against another revision is refused.
//! assert!(out.document.apply(&patch).is_err());
//! ```
//!
//! The crate is portable: no filesystem, clock, network or
//! threads, so it builds for `wasm32-unknown-unknown` unchanged.

// The README's examples are compiled and run by `cargo test` through this
// doctest-only module; it adds nothing to the rendered documentation.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}

mod body;
mod document;
mod emit;
mod fence;
mod frontmatter;
mod patch;
mod span;
mod utf16;
mod value;

pub use body::{BodyIndex, BodyLink, FootnoteDef, FootnoteRef, InlineFootnote};
pub use document::Document;
pub use emit::{entry as emit_entry, scalar_text as emit_scalar, Unrepresentable};
pub use fence::Fence;
pub use frontmatter::{BlockStyle, Entry, Frontmatter, FrontmatterLink, FrontmatterState};
pub use patch::{Applied, Edit, Patch, PatchError, PatchOp, Placement, Receipt};
pub use span::{SourceDigest, Span};
pub use utf16::Utf16Index;
pub use value::{Subpath, Value, Wikilink};
