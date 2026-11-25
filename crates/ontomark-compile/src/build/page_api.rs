//! `api/pages/<slug>.json` and `api/pages/_domain-index.json` — a port of
//! `pipeline/jsonld_to_page_api.py`.
//!
//! Each public page gets one closure-enriched JSON document. The twelve
//! `relationships` keys are **always present**, empty lists included: this is
//! the opposite of `scaffold-index.json`'s omit-empty rule, and both are v1
//! contracts that consumers already depend on.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::build::indexes::{inherited_to_json, refs_to_json};
use crate::ir::closure::Closure;
use crate::ir::model::{ClassRecord, Corpus, RELATION_KEYS};

/// The frozen `relationships` key order of the page API.
pub const PAGE_API_REL_ORDER: &[&str] = &[
    "hasPart",
    "requires",
    "enables",
    "dependsOn",
    "implements",
    "contrastsWith",
    "bridgesTo",
    "uses",
    "supports",
    "standardizedBy",
    "partOf",
    "relatedTo",
];

/// One page's API document, plus the slug it is written under.
#[derive(Debug, Clone)]
pub struct PageDocument {
    /// The file stem: `<slug>.json`.
    pub slug: String,
    /// The page the document was built from, named when two documents would
    /// claim one file.
    pub page_id: String,
    /// The document.
    pub value: Value,
}

/// Build every public page document and the domain index.
///
/// Returns `(documents, domain_index)`.
///
/// # Panics
/// Never: the domain index is only ever populated with JSON arrays, and the
/// `expect` asserts that invariant rather than hiding it.
#[must_use]
#[allow(clippy::implicit_hasher, clippy::too_many_lines)]
/// [`build_attributed`] with an empty roster.
pub fn build(
    corpus: &Corpus,
    closure: &Closure,
    backlinks: &HashMap<String, Vec<String>>,
) -> (Vec<PageDocument>, Value) {
    build_attributed(
        corpus,
        closure,
        backlinks,
        &super::attribution::Roster::default(),
    )
}

/// Version 3 page documents expose review freshness separately from historical protection.
/// The legacy page API remains unchanged; consumers opt into `api/v3/pages/`.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn build_v3(
    corpus: &Corpus,
    closure: &Closure,
    backlinks: &HashMap<String, Vec<String>>,
    roster: &super::attribution::Roster,
) -> (Vec<PageDocument>, Value) {
    build_v3_at(corpus, closure, backlinks, roster, None)
}

/// [`build_v3`] that also publishes a version 3 page's time and recall state:
/// `validFrom` and `validUntil` when it has them, and `recallable` (whether
/// current recall may return it at `now`, Unix seconds) for a durable memory
/// kind. Only a record built under vocabulary 3 carries those facts, so a
/// version 2 document is unchanged. `now` of `None` omits `recallable`.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn build_v3_at(
    corpus: &Corpus,
    closure: &Closure,
    backlinks: &HashMap<String, Vec<String>>,
    roster: &super::attribution::Roster,
    now: Option<i64>,
) -> (Vec<PageDocument>, Value) {
    let (mut documents, index) = build_attributed(corpus, closure, backlinks, roster);
    let records: HashMap<_, _> = corpus.public().map(|r| (r.page_id.as_str(), r)).collect();
    for doc in &mut documents {
        if let Some(record) = records.get(doc.page_id.as_str()) {
            doc.value["reviewBinding"] = json!({
                "current": record.certification.current,
                "stale": record.certification.stale,
                "unbound": record.certification.unbound,
            });
            if let Some(facts) = record.v3.as_deref() {
                // The instrument, in the v3 documents only: their context (v4)
                // has the term, and the legacy page API keeps its keys.
                if let (Some(via), Some(provenance)) = (
                    &facts.via,
                    doc.value
                        .get_mut("provenance")
                        .and_then(Value::as_object_mut),
                ) {
                    provenance.insert("via".into(), json!(via));
                }
                if let Some(from) = &facts.valid_from {
                    doc.value["validFrom"] = json!(from);
                }
                if let Some(to) = &facts.valid_to {
                    doc.value["validUntil"] = json!(to);
                }
                if let (true, Some(now)) = (facts.memory, now) {
                    doc.value["recallable"] = json!(facts.recallable_at(now));
                }
            }
        }
    }
    (documents, index)
}

/// Page documents with a `provenance` block when the page's generator resolves in the roster.
#[must_use]
#[allow(clippy::implicit_hasher, clippy::too_many_lines)]
pub fn build_attributed(
    corpus: &Corpus,
    closure: &Closure,
    backlinks: &HashMap<String, Vec<String>>,
    roster: &super::attribution::Roster,
) -> (Vec<PageDocument>, Value) {
    let public: Vec<&ClassRecord> = corpus.public().collect();
    let slug_to_title: HashMap<&str, &str> = public
        .iter()
        .map(|r| (r.slug.as_str(), r.title.as_str()))
        .collect();

    let mut domain_index: Map<String, Value> = Map::new();
    let mut documents = Vec::with_capacity(public.len());

    for record in &public {
        let mut entry = Map::new();
        entry.insert("id".into(), json!(record.page_iri));
        entry.insert("title".into(), json!(record.title));
        entry.insert("slug".into(), json!(record.slug));
        entry.insert("public".into(), json!(true));

        if record.has_ontology {
            entry.insert("classIri".into(), json!(record.iri));
            entry.insert("domain".into(), json!(record.domain));
            if let Some(gen) = record.generated_by.as_ref().and_then(|a| roster.resolve(a)) {
                entry.insert(
                    "provenance".into(),
                    json!({
                        "wasAttributedTo": gen.did,
                        "label": gen.label,
                        "actor": record.generated_by.as_ref().map(|a| a.as_string()),
                    }),
                );
            } else if let Some(principal) = record.v3.as_deref().and_then(|f| f.principal.as_ref())
            {
                // Vocabulary 3: `by` names a principal, which is not a roster
                // generator. The block keeps its keys; `label` is the roster's
                // name for the principal, else the DID.
                let label = roster
                    .principals()
                    .iter()
                    .find(|p| &p.did == principal)
                    .map_or(principal.as_str(), |p| p.label.as_str());
                entry.insert(
                    "provenance".into(),
                    json!({
                        "wasAttributedTo": principal,
                        "label": label,
                        "actor": record.generated_by.as_ref().map(|a| a.as_string()),
                    }),
                );
            }
            entry.insert(
                "certification".into(),
                json!({
                    "state": record.certification.state(),
                    "keys": record.certification.keys,
                }),
            );
            entry.insert("definition".into(), json!(record.definition));
            entry.insert("subClassOf".into(), refs_to_json(&record.sub_class_of));
            entry.insert("entityType".into(), json!(record.entity_type.as_str()));
            entry.insert("qualityScore".into(), json!(record.quality));
            entry.insert("maturity".into(), json!(record.maturity));

            let mut relationships = Map::new();
            for key in PAGE_API_REL_ORDER {
                let canonical = RELATION_KEYS
                    .iter()
                    .find(|(_, j)| j == key)
                    .map_or(*key, |(_, j)| *j);
                relationships.insert((*key).to_owned(), refs_to_json(record.relation(canonical)));
            }
            entry.insert("relationships".into(), Value::Object(relationships));

            let class_slug = record.class_slug();
            entry.insert(
                "inferredSuperClasses".into(),
                Value::Array(
                    closure
                        .inferred_of(&class_slug)
                        .iter()
                        .map(|a| {
                            json!({
                                "id": closure.iri_of(a),
                                "label": closure.label_of(a),
                                "slug": a,
                            })
                        })
                        .collect(),
                ),
            );
            entry.insert(
                "inheritedRelations".into(),
                closure
                    .inherited_relations
                    .get(&class_slug)
                    .map_or_else(|| Value::Object(Map::new()), inherited_to_json),
            );

            let domain = if record.domain.is_empty() {
                "unclassified"
            } else {
                record.domain.as_str()
            };
            domain_index
                .entry(domain.to_owned())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("domain index entries are arrays")
                .push(json!({
                    "slug": record.slug,
                    "title": record.label,
                    "qualityScore": record.quality,
                }));
        }

        entry.insert(
            "wikilinks".into(),
            Value::Array(
                record
                    .links
                    .iter()
                    .map(|r| json!({ "slug": r.slug(), "label": r.label }))
                    .collect(),
            ),
        );
        entry.insert(
            "backlinks".into(),
            Value::Array(
                backlinks
                    .get(&record.slug)
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .map(|s| {
                        json!({
                            "slug": s,
                            "label": slug_to_title.get(s.as_str()).copied().unwrap_or(s.as_str()),
                        })
                    })
                    .collect(),
            ),
        );

        documents.push(PageDocument {
            slug: record.slug.clone(),
            page_id: record.page_id.clone(),
            value: Value::Object(entry),
        });
    }

    (documents, Value::Object(domain_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::model::{EntityType, Ref};
    use indexmap::IndexMap;

    fn record(id: &str) -> ClassRecord {
        ClassRecord {
            page_id: id.to_owned(),
            slug: crate::input::slug::slugify(id),
            title: id.to_owned(),
            public: true,
            page_iri: format!("urn:visionflow:page:{}", crate::input::slug::slugify(id)),
            iri: format!("urn:ngm:class:{}", crate::input::slug::slugify(id)),
            label: id.to_owned(),
            entity_type: EntityType::Class,
            domain: "blockchain".into(),
            definition: "d".into(),
            maturity: "draft".into(),
            quality: 0.5,
            legacy_term_id: String::new(),
            attestations: Default::default(),
            generated_by: None,
            certification: crate::input::okf::Certification::default(),
            v3: None,
            sub_class_of: Vec::new(),
            instance_of: Vec::new(),
            relations: IndexMap::new(),
            links: Vec::new(),
            body: String::new(),
            has_ontology: true,
            defines_as: Vec::new(),
        }
    }

    fn corpus_of(records: Vec<ClassRecord>) -> Corpus {
        let mut by_class_slug = IndexMap::new();
        for (i, r) in records.iter().enumerate() {
            by_class_slug.entry(r.class_slug()).or_insert(i);
        }
        Corpus {
            records,
            by_class_slug,
        }
    }

    #[test]
    fn every_relationship_key_is_present_even_when_empty() {
        let (docs, _) = build(
            &corpus_of(vec![record("A")]),
            &Closure::default(),
            &HashMap::new(),
        );
        let rel = docs[0].value["relationships"].as_object().unwrap();
        assert_eq!(rel.len(), PAGE_API_REL_ORDER.len());
        let keys: Vec<&String> = rel.keys().collect();
        assert_eq!(keys, PAGE_API_REL_ORDER);
        assert_eq!(rel["hasPart"].as_array().unwrap(), &[] as &[Value]);
    }

    #[test]
    fn private_pages_get_no_document() {
        let mut r = record("Secret");
        r.public = false;
        let (docs, _) = build(&corpus_of(vec![r]), &Closure::default(), &HashMap::new());
        assert!(docs.is_empty());
    }

    #[test]
    fn the_domain_index_groups_by_domain() {
        let mut other = record("B");
        other.domain = String::new();
        let (_, index) = build(
            &corpus_of(vec![record("A"), other]),
            &Closure::default(),
            &HashMap::new(),
        );
        assert_eq!(index["blockchain"].as_array().unwrap().len(), 1);
        assert_eq!(index["unclassified"][0]["slug"], "b");
    }

    #[test]
    fn closure_enrichment_lands_on_the_document() {
        let mut closure = Closure::default();
        closure
            .inferred_superclasses
            .insert("a".into(), vec!["ancestor".into()]);
        closure.labels.insert("ancestor".into(), "Ancestor".into());
        let mut inherited = IndexMap::new();
        inherited.insert(
            "requires".to_owned(),
            vec![Ref {
                iri: "urn:ngm:class:x".into(),
                label: "X".into(),
            }],
        );
        closure.inherited_relations.insert("a".into(), inherited);

        let (docs, _) = build(&corpus_of(vec![record("A")]), &closure, &HashMap::new());
        let v = &docs[0].value;
        assert_eq!(v["inferredSuperClasses"][0]["slug"], "ancestor");
        assert_eq!(v["inferredSuperClasses"][0]["label"], "Ancestor");
        assert_eq!(v["inheritedRelations"]["requires"][0]["label"], "X");
    }

    #[test]
    fn backlinks_resolve_titles_and_fall_back_to_the_slug() {
        let mut backlinks = HashMap::new();
        backlinks.insert("a".to_owned(), vec!["b".to_owned(), "ghost".to_owned()]);
        let (docs, _) = build(
            &corpus_of(vec![record("A"), record("B")]),
            &Closure::default(),
            &backlinks,
        );
        let bl = docs[0].value["backlinks"].as_array().unwrap();
        assert_eq!(bl[0]["label"], "B");
        assert_eq!(bl[1]["label"], "ghost");
    }

    #[test]
    fn a_plain_note_gets_identity_and_links_only() {
        let mut r = record("Note");
        r.has_ontology = false;
        let (docs, index) = build(&corpus_of(vec![r]), &Closure::default(), &HashMap::new());
        let obj = docs[0].value.as_object().unwrap();
        assert!(!obj.contains_key("relationships"));
        assert!(obj.contains_key("wikilinks"));
        assert!(index.as_object().unwrap().is_empty());
    }

    /// ADR-OM-012: the v2 context is closed over the page API document.
    /// Every top-level key, every `relationships` key and every key of the
    /// `certification` and link objects must expand to an IRI.
    #[test]
    fn context_v2_has_a_term_for_every_page_api_key() {
        let mut yaml = String::from("version: 2\nnamespace: \"urn:ngm:class:\"\nrelations:\n");
        for (fm, json) in RELATION_KEYS {
            yaml.push_str(&format!(
                "  {fm}: {{ owl: \"vc:{json}\", json_key: {json} }}\n"
            ));
        }
        yaml.push_str("scalars:\n  domain: { type: text, owl: vc:sourceDomain }\n");
        let vocab = crate::input::vocabulary::Vocabulary::from_yaml_str(&yaml).unwrap();
        let profile = crate::profile::testing::est();
        let ctx = super::super::okf::context_v2(profile, &vocab);
        let terms = ctx["@context"].as_object().unwrap();

        let mut r = record("A");
        r.relations.insert(
            "requires",
            vec![Ref {
                iri: "urn:ngm:class:b".into(),
                label: "B".into(),
            }],
        );
        r.certification.keys = vec!["is-a".into()];
        let mut closure = Closure::default();
        closure
            .inferred_superclasses
            .insert("a".into(), vec!["b".into()]);
        let (docs, _) = build(&corpus_of(vec![r]), &closure, &HashMap::new());
        let doc = docs[0].value.as_object().unwrap();
        for key in doc.keys() {
            assert!(terms.contains_key(key), "page API key {key} has no v2 term");
        }
        for key in doc["relationships"].as_object().unwrap().keys() {
            assert!(terms.contains_key(key), "relationship {key} has no v2 term");
        }
        let scoped = terms["certification"]["@context"].as_object().unwrap();
        for key in doc["certification"].as_object().unwrap().keys() {
            assert!(scoped.contains_key(key), "certification.{key} has no term");
        }
        for key in ["id", "label", "slug"] {
            assert!(terms.contains_key(key), "link-object key {key} has no term");
        }
        assert_eq!(doc["certification"]["state"], "partial");
        assert_eq!(terms["domain"], "vc:sourceDomain");
        assert_eq!(terms["qualityScore"]["@type"], "xsd:float");
    }

    /// Version 3: the page document carries its time and recall state, the
    /// v4 context has a term for each new key, and a version 2 record gains
    /// nothing.
    #[test]
    fn version_3_documents_carry_time_and_recall_and_context_v4_covers_them() {
        use crate::ir::model::V3Facts;
        let facts = |status: &str, to: Option<&str>| {
            Some(Box::new(V3Facts {
                status: Some(status.into()),
                valid_from: Some("2026-01-01T00:00:00Z".into()),
                valid_to: to.map(str::to_owned),
                memory: true,
                type_name: Some("Belief".into()),
                ..V3Facts::default()
            }))
        };
        let mut live = record("Live");
        live.v3 = facts("approved", None);
        let mut closed = record("Closed");
        closed.v3 = facts("approved", Some("2026-11-21T10:00:00Z"));
        let mut draft = record("Draft");
        draft.v3 = facts("draft", None);
        let plain = record("Plain");
        let now = crate::input::time::parse_utc_seconds("2026-11-25T10:00:00Z");
        let (docs, _) = build_v3_at(
            &corpus_of(vec![live, closed, draft, plain]),
            &Closure::default(),
            &HashMap::new(),
            &super::super::attribution::Roster::default(),
            now,
        );
        let by = |slug: &str| docs.iter().find(|d| d.slug == slug).unwrap().value.clone();
        assert_eq!(by("live")["recallable"], true);
        assert_eq!(by("closed")["recallable"], false);
        assert_eq!(by("closed")["validUntil"], "2026-11-21T10:00:00Z");
        assert_eq!(by("draft")["recallable"], false);
        assert_eq!(by("live")["validFrom"], "2026-01-01T00:00:00Z");
        assert!(by("plain").get("recallable").is_none() && by("plain").get("validFrom").is_none());
        // Without a clock the flag is omitted rather than guessed.
        let mut again = record("Live");
        again.v3 = facts("approved", None);
        let (undated, _) = build_v3(
            &corpus_of(vec![again]),
            &Closure::default(),
            &HashMap::new(),
            &super::super::attribution::Roster::default(),
        );
        assert!(undated[0].value.get("recallable").is_none());

        let vocab = crate::input::vocabulary::Vocabulary::from_yaml_str(
            "version: 3\nnamespace: \"urn:ngm:class:\"\n",
        )
        .unwrap();
        let ctx = super::super::okf::context_v4(crate::profile::testing::est(), &vocab);
        let terms = ctx["@context"].as_object().unwrap();
        for key in by("closed").as_object().unwrap().keys() {
            assert!(terms.contains_key(key), "page API key {key} has no v4 term");
        }
        assert_eq!(terms["validFrom"]["@id"], "temporal:validFrom");
        assert_eq!(terms["temporal"], "http://nova.dev/temporal#");
        assert!(terms.contains_key("reviewBinding"), "v4 keeps v3's terms");
    }
}
