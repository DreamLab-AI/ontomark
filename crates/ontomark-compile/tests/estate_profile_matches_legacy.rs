//! The estate profile's data equals the constants the relocated pipeline
//! still compiles in. Deleted once the pipeline reads only the profile.

use ontomark_compile::profile::Profile;

fn estate() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml")).unwrap()
}

#[test]
fn taxonomy_equals_graph_tiers_tables() {
    use ontomark_compile::build::graph_tiers::CATEGORY_ORDER;
    use ontomark_compile::input::domains::{domain_class_id, DOMAIN_ROOTS};
    let p = estate();
    let cats: Vec<(&str, &str, u16)> = p
        .taxonomy
        .categories
        .iter()
        .map(|c| (c.slug.as_str(), c.label.as_str(), c.domain))
        .collect();
    assert_eq!(cats, CATEGORY_ORDER);
    let doms: Vec<(&str, &str)> = p
        .taxonomy
        .domains
        .iter()
        .map(|d| (d.slug.as_str(), d.label.as_str()))
        .collect();
    assert_eq!(doms, DOMAIN_ROOTS);
    for probe in ["ai", "bc", "mv", "rb", "ngm", "tc", "robotics", "geodesy", ""] {
        assert_eq!(p.taxonomy.domain_class_id(probe), domain_class_id(probe), "{probe}");
    }
}

#[test]
fn iris_equal_turtle_constants() {
    use ontomark_compile::build::turtle::{MATURITY_LEVELS, NGM, NGMI, ONTOLOGY_IRI, VC, VERSION_INFO};
    let p = estate();
    assert_eq!(p.iris.public_class_base, NGM);
    assert_eq!(p.iris.public_individual_base, NGMI);
    assert_eq!(p.iris.vocabulary_namespace, VC);
    assert_eq!(p.iris.ontology_iri, ONTOLOGY_IRI);
    assert_eq!(p.iris.version_info, VERSION_INFO);
    let m: Vec<(&str, &str)> = p
        .lifecycle
        .maturity_levels
        .iter()
        .map(|t| (t.slug.as_str(), t.label.as_str()))
        .collect();
    assert_eq!(m, MATURITY_LEVELS);
    assert_eq!(
        p.dataset.attributed_to,
        ontomark_compile::build::graph_tiers::ATTRIBUTED_TO
    );
}
