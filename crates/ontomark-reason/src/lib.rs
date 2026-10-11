//! OWL 2 EL reasoning for ontomark: a profile check that refuses what EL
//! cannot express (and says why), and a classification facade over
//! [`whelk_el`] that reports inferred subsumptions, unsatisfiable classes,
//! their root causes and the disjointness axioms behind them.
//!
//! The crate takes logical axioms ([`axiom::Axiom`]), never Markdown; it does
//! not depend on the parser. It runs unchanged on `wasm32-unknown-unknown`.
//!
//! ```
//! use ontomark_reason::axiom::{Axiom, ClassExpression as C};
//! use ontomark_reason::classify;
//!
//! let axioms = vec![
//!     Axiom::sub_class("urn:P", "urn:A"),
//!     Axiom::sub_class("urn:P", "urn:B"),
//!     Axiom::DisjointClasses(vec![C::class("urn:A"), C::class("urn:B")]),
//!     Axiom::SubClassOf {
//!         sub: C::class("urn:Q"),
//!         sup: C::Only { property: ontomark_reason::axiom::Property::named("urn:r"), filler: Box::new(C::class("urn:A")) },
//!     },
//! ];
//! let result = classify(&axioms);
//! assert_eq!(result.unsatisfiable, vec!["urn:P".to_owned()]);
//! assert_eq!(result.disjointness_violations[0].disjoint, ("urn:A".to_owned(), "urn:B".to_owned()));
//! // The universal restriction is outside EL: refused, named, and the result
//! // says it is incomplete.
//! assert_eq!(result.refusals[0].construct, "ObjectAllValuesFrom");
//! assert!(!result.is_complete());
//! ```
//!
//! For long-running work in a host that must stay responsive, and whose
//! source changes underneath the reasoner, use [`session`].

pub mod axiom;
pub mod classify;
pub mod profile;
pub mod session;

pub use classify::{
    classify, classify_with, Classification, DisjointnessViolation, ExistentialPolicy, Options,
    Stopped, PROFILE,
};
pub use profile::{check, Refusal, RefusalKind};
