//! `ontomark-write`: the headless write engine for working-memory pages.
//!
//! ADR-VG-012 section 9 asks for one write path: an agent's page obeys exactly
//! the rules of the operator's editor. This crate is that path. Every
//! operation takes a snapshot of the vault, plans the write, validates the
//! result with the same validator `ontomark validate` runs, and returns a
//! [`Plan`]; nothing is written until [`Plan::commit`] applies it to a
//! [`Store`], atomically across every file the operation touches.
//!
//! | operation | what it does |
//! |---|---|
//! | [`Engine::create`] | a new working page: minted `page_resource`, first `generated` stamp, visibility, initial status |
//! | [`Engine::patch`] | change only the named keys; every other byte is identical; contributors and clocks stamped |
//! | [`Engine::adopt`] | give an existing page (a daily note) the identity it lacks |
//! | [`Engine::transition`] | approve, reject, confirm, send back, withdraw, archive, per the ADR-VG-013 table |
//! | [`Engine::record_disputes`] | a draft names the approved pages it contradicts |
//! | [`Engine::complete_supersession`] | finish an approve-then-supersede that was interrupted |
//!
//! Approving a disputed draft is one operation over several pages: the draft
//! is written first (`approved`), then each page it replaces (`superseded`),
//! and [`Plan::commit`] writes all of them or none.
//!
//! # Vocabulary versions
//!
//! Under vocabulary 3 every operation applies. Under vocabulary 2 only
//! [`Engine::patch`] works, as a lossless pass-through (no stamps, no new
//! validation); the rest are refused with [`code::VOCABULARY_V2`], because
//! stamps, clocks, the lifecycle and `page_resource` form are version 3
//! concepts the version 2 validator was never specified to judge.
//!
//! # Portability
//!
//! The core does no I/O, reads no clock and draws no randomness: the host
//! passes `now` and 32 bytes of entropy. It builds for `wasm32-unknown-unknown`
//! with `--no-default-features`. The default features add [`FsStore`] and the
//! [`system`] clock and entropy for native hosts.
//!
//! ```
//! use ontomark_compile::input::page::{Vault, VaultKind};
//! use ontomark_compile::input::vocabulary::Vocabulary;
//! use ontomark_compile::profile::Profile;
//! use ontomark_write::{CreateRequest, Engine, MemStore, Writer};
//!
//! let vocab = Vocabulary::from_yaml_str(
//!     "version: 3\nworking_types:\n  Note: {}\n",
//! )
//! .unwrap();
//! let working = Vault {
//!     root: "/v/working".into(),
//!     kind: VaultKind::Working,
//!     pages: vec![],
//!     journals: vec![],
//!     skipped: vec![],
//! };
//! let engine = Engine::new(Profile::default(), vocab, None, working, None);
//!
//! let plan = engine
//!     .create(&CreateRequest {
//!         id: "notes/Tea first".into(),
//!         page_type: "Note".into(),
//!         fields: vec![("tags".into(), ontomark_parse::Value::strings(["drinks"]))],
//!         body: "Tea before coffee.\n".into(),
//!         visibility: None,
//!         writer: Writer::new(
//!             "did:nostr:b41654017f6850b13857d19d8ae0e3f88f1365600cab0321e8101c9e92682f7a",
//!         ),
//!         now: "2026-11-25T12:00:00Z".into(),
//!         entropy: [7; 32],
//!     })
//!     .unwrap();
//! let mut store = MemStore::default();
//! plan.commit(&mut store).unwrap();
//! let text = store.text(VaultKind::Working, "notes/Tea first.md").unwrap();
//! assert!(text.contains("status: draft"));
//! assert!(text.contains("page_resource: urn:visionflow:page:0707"));
//! assert!(text.ends_with("Tea before coffee.\n"));
//! ```

mod actor;
mod edit;
mod engine;
mod error;
#[cfg(feature = "fs")]
mod fs;
mod keys;
mod ops;
mod plan;
mod store;
#[cfg(feature = "system")]
pub mod system;
pub mod wire;

pub use actor::{check_timestamp, Actor, Writer};
pub use engine::Engine;
pub use error::{code, IssueRecord, Refusal};
#[cfg(feature = "fs")]
pub use fs::FsStore;
pub use keys::{is_content_key, CLOCK_KEYS, IMMUTABLE_KEYS, LIFECYCLE_KEYS};
pub use ops::adopt::AdoptRequest;
pub use ops::create::{CreateRequest, PAGE_URN};
pub use ops::disputes::{DisputeInput, RecordDisputesRequest};
pub use ops::patch::{BodyEdit, Change, PatchRequest};
pub use ops::supersede::{CompleteRequest, Pending, Resolution, SupersedeReason};
pub use ops::transition::{Move, TransitionRequest, Withdrawal};
pub use ops::CONTRIBUTORS_ONE_PER_WRITER;
pub use plan::{DisputeRef, Plan, PlannedWrite, ReviewEvent, ReviewEventKind};
pub use store::{CommitError, CommitReport, MemStore, Store, WriteMode};

use ontomark_compile::input::page::VaultKind;
use serde::{Deserialize, Serialize};

/// Which vault a page is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// `working/`, the shared working memory.
    #[default]
    Working,
    /// `knowledge/`, the governed bundle.
    Knowledge,
}

impl Role {
    /// The compile crate's name for the role.
    #[must_use]
    pub fn kind(self) -> VaultKind {
        match self {
            Self::Working => VaultKind::Working,
            Self::Knowledge => VaultKind::Knowledge,
        }
    }
}
