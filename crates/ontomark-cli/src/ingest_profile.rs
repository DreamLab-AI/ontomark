//! The typed-model profile `ingest` reads a vault through.
//!
//! `ontomark-ingest` works on [`ontomark_model::Profile`], while the CLI
//! loads vaults through the compiler's [`Vocabulary`]. For an estate-shaped
//! repository this module derives the model profile from that vocabulary, so
//! the keys a model may fill are exactly the `core` relations the vocabulary
//! declares (which `validate` accepts on a knowledge page) and entity IRIs
//! come from `resource`. A plain vault, which has no relations, reads as
//! `generic/1`.

use anyhow::Context as _;
use ontomark_compile::input::vocabulary::{RelationStatus, Vocabulary};
use ontomark_model::{
    DeclarativeProfile, KindSpec, PredicateForm, PredicateSpec, ProfileSpec, SkosRelation,
};

/// The form a relation's OWL property gives its key.
fn form_of(owl: &str) -> PredicateForm {
    match owl {
        "rdfs:subClassOf" => PredicateForm::SubClassOf,
        "rdf:type" => PredicateForm::ClassAssertion,
        "owl:equivalentClass" => PredicateForm::EquivalentClass,
        "owl:disjointWith" => PredicateForm::DisjointWith,
        "owl:sameAs" => PredicateForm::SameAs,
        "rdfs:subPropertyOf" => PredicateForm::SubPropertyOf,
        "skos:broader" => PredicateForm::Skos(SkosRelation::Broader),
        "skos:narrower" => PredicateForm::Skos(SkosRelation::Narrower),
        "skos:related" => PredicateForm::Skos(SkosRelation::Related),
        "skos:exactMatch" => PredicateForm::Skos(SkosRelation::ExactMatch),
        "skos:closeMatch" => PredicateForm::Skos(SkosRelation::CloseMatch),
        // A domain relation between two classes is a restriction on the
        // subject class; on an individual it is an object assertion.
        _ => PredicateForm::Existential,
    }
}

fn kind_of(owl: &str) -> Option<KindSpec> {
    match owl {
        "owl:Class" => Some(KindSpec::Class),
        "owl:NamedIndividual" => Some(KindSpec::Individual),
        "owl:ObjectProperty" => Some(KindSpec::ObjectProperty),
        "owl:DatatypeProperty" => Some(KindSpec::DataProperty),
        "owl:AnnotationProperty" => Some(KindSpec::AnnotationProperty),
        _ => None,
    }
}

/// Derive the model profile for `vocab`.
///
/// # Errors
/// When the vocabulary's prefixes or relation properties are not IRIs.
pub fn build(vocab: &Vocabulary) -> anyhow::Result<DeclarativeProfile> {
    let relations: Vec<_> = vocab
        .relations
        .iter()
        .filter(|(_, r)| r.status == RelationStatus::Core)
        .collect();
    if relations.is_empty() {
        return Ok(DeclarativeProfile::generic());
    }
    let spec = ProfileSpec {
        id: "ontomark-cli/ingest-vocabulary/1".into(),
        prefixes: vocab
            .prefixes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        type_key: "type".into(),
        kinds: vocab
            .types
            .iter()
            .filter_map(|(name, t)| kind_of(&t.owl).map(|k| (name.clone(), k)))
            .collect(),
        untyped: KindSpec::NotOntology,
        identity_key: "resource".into(),
        title_key: "title".into(),
        aliases_key: "aliases".into(),
        public_key: "public".into(),
        lenient_booleans: true,
        lifecycle_key: "status".into(),
        lifecycle: Vec::new(),
        predicates: relations
            .into_iter()
            .map(|(key, r)| {
                (
                    key.clone(),
                    PredicateSpec {
                        iri: r.owl.clone(),
                        form: form_of(&r.owl),
                        datatype: None,
                        domain: None,
                        range: None,
                        transitive: false,
                    },
                )
            })
            .collect(),
        curie_keys: false,
        reserved: Vec::new(),
        reference_namespaces: Vec::new(),
        publication: None,
        cycle_policy: ontomark_model::CyclePolicy::Warn,
        mint: None,
    };
    DeclarativeProfile::new(spec).context("deriving the ingest profile from the vocabulary")
}
