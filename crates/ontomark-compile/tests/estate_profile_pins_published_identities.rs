//! The estate profile pins the identities the corpus has published. Domain and
//! category positions are NGG1 ids and GPU class ids; a reorder, an insertion
//! before the end or a rename changes bytes consumers already hold, so every
//! value below is a literal and a change here is a breaking change.

use ontomark_compile::profile::Profile;

fn estate() -> Profile {
    Profile::from_yaml(include_str!("../../../profiles/estate/profile.yaml")).unwrap()
}

#[test]
fn domains_keep_their_publication_order() {
    let p = estate();
    let domains: Vec<(&str, &str)> = p
        .taxonomy
        .domains
        .iter()
        .map(|d| (d.slug.as_str(), d.label.as_str()))
        .collect();
    assert_eq!(
        domains,
        [
            ("artificial-intelligence", "Artificial Intelligence"),
            ("blockchain", "Blockchain"),
            ("spatial-computing", "Spatial Computing"),
            ("robotics", "Robotics"),
            ("distributed-collaboration", "Distributed Collaboration"),
            ("infrastructure", "Infrastructure"),
            ("space-science-and-systems", "Space Science and Systems"),
            (
                "earth-observation-and-geospatial-sensing",
                "Earth Observation and Geospatial Sensing"
            ),
            ("supply-chains-and-logistics", "Supply Chains and Logistics"),
        ]
    );
}

#[test]
fn categories_keep_their_ids_labels_and_domains() {
    let p = estate();
    let cats: Vec<(&str, &str, u16)> = p
        .taxonomy
        .categories
        .iter()
        .map(|c| (c.slug.as_str(), c.label.as_str(), c.domain))
        .collect();
    assert_eq!(
        cats,
        [
            ("ai-technique", "AI Technique", 0),
            ("ai-model-architecture", "AI Model Architecture", 0),
            ("ai-application", "AI Application", 0),
            ("ai-governance-and-ethics", "AI Governance and Ethics", 0),
            ("cat-ai-infrastructure", "AI Infrastructure", 0),
            ("ai-research-area", "AI Research Area", 0),
            ("bc-protocol-and-consensus", "Protocol and Consensus", 1),
            ("bc-cryptographic-primitive", "Cryptographic Primitive", 1),
            ("bc-token-and-asset", "Token and Asset", 1),
            ("bc-defi-and-economics", "DeFi and Economics", 1),
            ("bc-network-component", "Network Component", 1),
            (
                "bc-governance-and-regulation",
                "Governance and Regulation",
                1
            ),
            ("sc-display-and-rendering", "Display and Rendering", 2),
            ("sc-interaction", "Interaction Technology", 2),
            ("sc-content-and-assets", "Content and Assets", 2),
            ("sc-platform-and-environment", "Platform and Environment", 2),
            (
                "sc-standards-and-interop",
                "Standards and Interoperability",
                2
            ),
            ("sc-governance-and-safety", "Governance and Safety", 2),
            ("robo-perception", "Perception and Sensing", 3),
            ("robo-actuation-and-control", "Actuation and Control", 3),
            ("robo-robot-type", "Robot Type", 3),
            ("robo-navigation-and-planning", "Navigation and Planning", 3),
            ("robo-safety-and-standards", "Safety and Standards", 3),
            ("robo-human-robot-interaction", "Human-Robot Interaction", 3),
            ("dc-communication", "Communication Technology", 4),
            ("dc-workspace-tools", "Workspace Tools", 4),
            ("dc-telepresence", "Telepresence", 4),
            ("dc-protocol-and-infra", "Protocol and Infrastructure", 4),
            ("infra-computing-and-cloud", "Computing and Cloud", 5),
            ("infra-network-and-comms", "Network and Communication", 5),
            ("infra-security-and-identity", "Security and Identity", 5),
            ("infra-data-management", "Data Management", 5),
            ("infra-legal-and-regulatory", "Legal and Regulatory", 5),
            ("infra-software-engineering", "Software Engineering", 5),
            ("astronomy", "Astronomy", 6),
            ("heliophysics", "Heliophysics", 6),
            ("planetary-science", "Planetary Science", 6),
            ("orbital-dynamics", "Orbital Dynamics", 6),
            ("space-reference-systems", "Space Reference Systems", 6),
            (
                "positioning-navigation-and-timing",
                "Positioning, Navigation and Timing",
                6
            ),
            ("spacecraft-bus", "Spacecraft Systems", 6),
            ("spacecraft-propulsion", "Spacecraft Propulsion", 6),
            (
                "spacecraft-power-and-thermal-control",
                "Spacecraft Power and Thermal Control",
                6
            ),
            (
                "spacecraft-guidance-navigation-and-control",
                "Guidance, Navigation and Control",
                6
            ),
            ("spacecraft-communications", "Satellite Communications", 6),
            ("space-life-sciences", "Space Life Sciences", 6),
            ("space-prime", "Space Industry", 6),
            ("earth-observation-sensor", "Observation Methods", 7),
            ("earth-system-phenomena", "Observed Phenomena", 7),
            ("earth-observation-processing", "Data Processing", 7),
            ("earth-observation-data-quality", "Data Quality", 7),
            ("earth-observation-applications", "Applications", 7),
            ("geodesy", "Geodesy", 7),
            ("supply-chain-strategy", "Strategy and Network Design", 8),
            ("supply-chain-planning", "Planning", 8),
            ("sourcing-and-procurement", "Sourcing and Procurement", 8),
            ("production-operations", "Production", 8),
            ("inventory-management", "Inventory", 8),
            ("warehousing-and-distribution", "Warehousing and Distribution", 8),
            ("freight-transportation", "Transportation", 8),
            ("international-trade-and-customs", "Trade and Customs", 8),
            ("supply-chain-risk-and-resilience", "Risk and Resilience", 8),
            ("sustainable-supply-chains", "Sustainability", 8),
            ("traceability-and-product-identification", "Traceability", 8),
            ("supply-chain-performance-and-finance", "Performance and Finance", 8),
            ("supply-chain-information-systems", "Information Systems", 8),
            ("supply-chain-organisations", "Organisations", 8),
        ]
    );
}

#[test]
fn short_names_and_aliases_resolve_to_the_published_class_ids() {
    let t = estate().taxonomy;
    for (probe, id) in [
        ("ai", 1),
        ("artificial-intelligence", 1),
        ("bc", 2),
        ("blockchain", 2),
        ("mv", 3),
        ("spatial-computing", 3),
        ("rb", 4),
        ("robotics", 4),
        ("ngm", 5),
        ("distributed-collaboration", 5),
        ("tc", 6),
        ("infrastructure", 6),
        ("space-science-and-systems", 7),
        ("earth-observation-and-geospatial-sensing", 8),
        ("supply-chains-and-logistics", 9),
        ("supply-chain", 9),
        ("geodesy", 0),
        ("", 0),
    ] {
        assert_eq!(t.domain_class_id(probe), id, "{probe}");
    }
}

#[test]
fn identity_namespaces_and_their_publication() {
    let r = estate().iris;
    assert_eq!(r.class_namespace, "urn:ngm:class:");
    assert_eq!(r.individual_namespace, "urn:ngm:individual:");
    assert_eq!(r.page_namespace, "urn:visionflow:page:");
    assert_eq!(r.public_class_base, "https://narrativegoldmine.com/class/");
    assert_eq!(
        r.public_individual_base,
        "https://narrativegoldmine.com/individual/"
    );
    assert_eq!(r.public_page_base, "https://narrativegoldmine.com/page/");
    assert_eq!(
        r.vocabulary_namespace,
        "https://narrativegoldmine.com/ns/v1#"
    );
    assert_eq!(r.ns_base, "https://narrativegoldmine.com/ns");
    assert_eq!(r.ontology_iri, "https://narrativegoldmine.com/ontology");
    assert_eq!(
        r.inferred_ontology_iri,
        "https://narrativegoldmine.com/ontology/inferred"
    );
    assert_eq!(r.version_info, "3.1.0");
    assert_eq!(
        (
            r.vocabulary_prefix.as_str(),
            r.class_prefix.as_str(),
            r.individual_prefix.as_str()
        ),
        ("vc", "ngm", "ngmi")
    );
}

#[test]
fn maturity_levels_and_dataset_identity() {
    let p = estate();
    let m: Vec<(&str, &str)> = p
        .lifecycle
        .maturity_levels
        .iter()
        .map(|t| (t.slug.as_str(), t.label.as_str()))
        .collect();
    assert_eq!(
        m,
        [
            ("established", "Established"),
            ("emerging", "Emerging"),
            ("draft", "Draft"),
            ("stub", "Stub"),
            ("deprecated", "Deprecated"),
        ]
    );
    assert_eq!(p.lifecycle.default_maturity, "draft");
    assert_eq!(p.dataset.pipeline_version, "ng-1.0.0");
    assert_eq!(
        p.dataset.attributed_to,
        "did:nostr:d598abc6edefc37e10523aec4c00f8d9674a93791aa34a1db03bfa9b60b9f246"
    );
    assert_eq!(p.dataset.attributed_label, "DreamLab AI");
}
