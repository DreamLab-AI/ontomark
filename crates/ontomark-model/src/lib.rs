//! Typed ontology model derived from Markdown pages (ADR-2015).
//!
//! `ontomark-model` reads [`ontomark_parse::Document`]s through a
//! [`Profile`] and produces a [`Model`]:
//!
//! * **identity is an IRI**, stored verbatim from the page header and never
//!   minted, re-prefixed or derived from a title; the document id, label,
//!   aliases and publication URL are separate fields;
//! * **kinds are explicit** — [`Kind::Class`], [`Kind::Individual`] and
//!   [`Kind::Property`] (object, data, annotation); an undeclared type is
//!   refused, never coerced to a class;
//! * **predicates are full IRIs** chosen by the profile per key, each with
//!   one [`PredicateForm`]: subclass, existential restriction, class or
//!   object or data assertion, annotation, SKOS relation, equivalence,
//!   disjointness, sameness or sub-property;
//! * **references resolve to one of four states** — [`Resolution::Resolved`],
//!   [`Resolution::Ambiguous`], [`Resolution::Unresolved`] or
//!   [`Resolution::External`] — never to a first match or a synthesised
//!   target;
//! * every statement carries a [`Disposition`] saying whether it is in the
//!   supported reasoning fragment, preserved without reasoning, or rejected;
//! * corpus policies such as subclass acyclicity are [`PolicyFinding`]s, not
//!   logical inconsistencies.
//!
//! The model is derived and read-only; edits go back to Markdown through
//! [`ontomark_parse::Patch`].
//!
//! ```
//! use ontomark_model::{DeclarativeProfile, DocId, Kind, Model, PropertyKind};
//! use ontomark_parse::Document;
//!
//! let p = Document::parse("---\ntype: ObjectProperty\niri: https://ex.org/partOf\n---\n");
//! let profile = DeclarativeProfile::generic();
//! let m = Model::build(&profile, [(DocId::new("partOf.md"), &p)]);
//! let e = m.entities.values().next().unwrap();
//! assert_eq!(e.kind, Kind::Property(PropertyKind::Object));
//! ```

mod iri;
mod kind;
mod model;
mod policy;
mod profile;

pub use iri::{Iri, IriError, PrefixMap};
pub use kind::{Kind, PropertyKind};
pub use model::Disposition;
pub use model::{
    codes, Diagnostic, DocDisposition, DocId, DocumentRecord, Entity, Literal, Model, Object,
    Origin, PolicyFinding, Reference, Resolution, Severity, Statement, StatementForm,
};
pub use profile::{
    CyclePolicy, DeclarativeProfile, IdentityDecision, KindDecision, KindSpec, Lifecycle,
    LifecycleDef, PredicateDef, PredicateForm, PredicateSpec, Profile, ProfileError, ProfileSpec,
    PublicationSpec, SkosRelation,
};
