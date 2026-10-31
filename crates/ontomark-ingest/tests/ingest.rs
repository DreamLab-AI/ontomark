//! The plan/accept contract end to end, over a small synthetic vault.

use ontomark_ingest::{
    AliasTier, Candidates, Grounding, Ingest, IngestError, ModelRequest, ModelResponse, Options,
    ProposedAssertion, ValueKind, WithholdReason, CONFIDENCE, CONTRACT, METHOD,
};
use ontomark_model::{DeclarativeProfile, DocId, Iri, Kind, Model, PredicateForm, ProfileSpec};
use ontomark_parse::{Document, Span, Value};

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

fn profile() -> DeclarativeProfile {
    // A JSON string, not `json!`, so predicate declaration order survives.
    let spec: ProfileSpec = serde_json::from_str(&format!(
        r##"{{
        "id": "ingest-test/1",
        "kinds": {{ "Class": "class", "NamedIndividual": "individual" }},
        "predicates": {{
            "is-a": {{ "iri": "http://www.w3.org/2000/01/rdf-schema#subClassOf", "form": "sub_class_of" }},
            "part-of": {{ "iri": "urn:x:partOf", "form": "existential" }},
            "founded": {{ "iri": "urn:x:founded", "form": "data_assertion", "datatype": "{XSD}date" }},
            "population": {{ "iri": "urn:x:population", "form": "data_assertion", "datatype": "{XSD}integer" }},
            "weight": {{ "iri": "urn:x:weight", "form": "data_assertion", "datatype": "{XSD}decimal" }},
            "note": {{ "iri": "urn:x:note", "form": "annotation" }}
        }},
        "reserved": ["tags", "colour"]
    }}"##
    ))
    .unwrap();
    DeclarativeProfile::new(spec).unwrap()
}

struct Vault {
    docs: Vec<(DocId, Document)>,
}

impl Vault {
    fn new() -> Self {
        let page = |name: &str, front: &str, body: &str| {
            (
                DocId::new(format!("{name}.md")),
                Document::parse(format!("---\n{front}\n---\n{body}")),
            )
        };
        Self {
            docs: vec![
                page(
                    "Graph",
                    "type: Class\niri: urn:x:Graph\nfounded: 2001-01-01",
                    "Founded on 2002-02-02.\n",
                ),
                page("Thing", "type: Class\niri: urn:x:Thing", ""),
                page(
                    "Knowledge graph",
                    "type: Class\niri: urn:x:KG\naliases:\n- KG\nfounded: 2019-03-01\nis-a:\n- \"[[Thing]]\"",
                    "A knowledge graph is a directed graph. It holds 1,200 nodes and was first built on 2019-03-01. Colour: indigo.\n",
                ),
                page(
                    "Dmitry Baranov",
                    "type: NamedIndividual\niri: urn:x:DB",
                    "",
                ),
                page("Company S", "type: NamedIndividual\niri: urn:x:CS", ""),
                page("Company T", "type: NamedIndividual\niri: urn:x:CT", ""),
                // One note whose own names differ only by an initial.
                page(
                    "Company X",
                    "type: NamedIndividual\niri: urn:x:CX\naliases:\n- Company Y",
                    "",
                ),
                page("Dup one", "type: Class\niri: urn:x:Dup\naliases:\n- Duplicate", ""),
                page("Dup two", "type: Class\niri: urn:x:Dup", ""),
                page(
                    "Lab",
                    "type: NamedIndividual\niri: urn:x:Lab",
                    "# People\nRun by Baranov D. since 2020.\n\n# Place\nThe lab is part of Company U. Baranov D. founded it.\n",
                ),
                // Two notes sharing a name: a bare [[Twin]] is ambiguous.
                page("a/Twin", "type: NamedIndividual\niri: urn:x:A\naliases:\n- Alpha", ""),
                page("b/Twin", "type: NamedIndividual\niri: urn:x:B\naliases:\n- Beta", ""),
                // Keys present with no value.
                page(
                    "Site",
                    "type: NamedIndividual\niri: urn:x:Site\npart-of:\nfounded:",
                    "Opened 2021-05-05. Hosted by Alpha and Beta. Weighs 3.5 or 4.5 tonnes.\n",
                ),
                page("Plain note", "tags: [x]", "Nothing ontological here.\n"),
            ],
        }
    }

    fn model(&self, profile: &DeclarativeProfile) -> Model {
        Model::build(profile, self.docs.iter().map(|(id, d)| (id.clone(), d)))
    }

    fn doc(&self, name: &str) -> &Document {
        &self
            .docs
            .iter()
            .find(|(id, _)| id.name() == name)
            .unwrap()
            .1
    }
}

fn iri(s: &str) -> Iri {
    Iri::parse(s).unwrap()
}

fn respond(key: &str, value: &str, evidence: &str) -> ProposedAssertion {
    ProposedAssertion::new(key, value, evidence)
}

fn reason<'a>(out: &'a Candidates, value: &str) -> &'a WithholdReason {
    &out.withheld
        .iter()
        .find(|w| w.value == value)
        .unwrap_or_else(|| panic!("{value} was not withheld: {out:#?}"))
        .reason
}

#[test]
fn plan_describes_keys_chunks_and_revision() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let kg = v.doc("Knowledge graph");
    let req = ingest.plan(&DocId::new("Knowledge graph.md"), kg).unwrap();

    assert_eq!(req.contract, CONTRACT);
    assert_eq!(req.note.iri, iri("urn:x:KG"));
    assert_eq!(req.note.kind, Kind::Class);
    assert_eq!(req.note.digest, kg.digest().hex());
    let keys: Vec<&str> = req.keys.iter().map(|k| k.key.as_str()).collect();
    assert_eq!(
        keys,
        ["is-a", "part-of", "founded", "population", "weight", "note"]
    );
    assert!(matches!(req.keys[0].value, ValueKind::Name));
    assert!(
        matches!(req.keys[2].value, ValueKind::Literal { ref datatype } if datatype.as_str().ends_with("#date"))
    );
    assert_eq!(req.keys[0].present, vec!["[[Thing]]".to_owned()]);
    assert_eq!(req.keys[2].present, vec!["2019-03-01".to_owned()]);
    assert_eq!(req.chunks.len(), 1);
    assert_eq!(req.chunks[0].text, kg.body());
    assert!(req.instructions.contains("Never answer with an identifier"));
    assert_eq!(
        req.response_schema["properties"]["assertions"]["items"]["properties"]["key"]["enum"],
        serde_json::json!(keys)
    );

    // Round trip as the wire format a host stores.
    let json = serde_json::to_string(&req).unwrap();
    let back: ModelRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back, req);
}

#[test]
fn plan_refuses_non_entities_and_keyless_profiles() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    assert_eq!(
        ingest.plan(&DocId::new("Plain note.md"), v.doc("Plain note")),
        Err(IngestError::NotAnEntity {
            document: DocId::new("Plain note.md")
        })
    );
    let generic = DeclarativeProfile::generic();
    let m = v.model(&generic);
    let ingest = Ingest::new(&generic, &m, Options::default());
    assert_eq!(
        ingest.plan(&DocId::new("Graph.md"), v.doc("Graph")),
        Err(IngestError::NoAssertionKeys {
            profile: "generic/1".into()
        })
    );
}

#[test]
fn recorded_response_yields_one_patch_and_named_refusals() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let kg = v.doc("Knowledge graph");
    let req = ingest.plan(&DocId::new("Knowledge graph.md"), kg).unwrap();
    let responses: Vec<ModelResponse> =
        serde_json::from_str(include_str!("fixtures/response-basic.json")).unwrap();
    let out = ingest.accept(kg, &req, &responses).unwrap();

    // Accepted: the grounded name and the repaired integer.
    let written: Vec<(&str, &str)> = out
        .accepted
        .iter()
        .map(|a| (a.key.as_str(), a.written.as_str()))
        .collect();
    assert_eq!(written, [("is-a", "[[Graph]]"), ("population", "1200")]);
    let graph = &out.accepted[0];
    assert_eq!(graph.grounded, Some(iri("urn:x:Graph")));
    assert_eq!(graph.tier, Some(AliasTier::Exact));
    assert_eq!(graph.confidence, CONFIDENCE);
    assert_eq!(graph.method, METHOD);
    assert_eq!(graph.evidence.len(), 1);
    let ev = &graph.evidence[0];
    assert_eq!(ev.span.slice(kg.source()), "is a directed graph");
    assert_eq!(ev.quote, "is a directed graph");
    let idx = kg.utf16_index();
    assert_eq!(idx.span(ev.span), Some((ev.utf16.start, ev.utf16.end)));
    assert!(out
        .diagnostics
        .iter()
        .any(|d| d.code == "literal_repaired" && d.message.contains("1,200")));

    // Withheld, each for its own reason.
    assert_eq!(reason(&out, "2019-03-01"), &WithholdReason::AlreadyPresent);
    assert_eq!(reason(&out, "indigo"), &WithholdReason::UnknownKey);
    assert_eq!(
        reason(&out, "a sentence that is not there"),
        &WithholdReason::EvidenceNotFound
    );
    match reason(&out, "Grph") {
        WithholdReason::Ungrounded { suggestions } => {
            assert_eq!(suggestions[0].iri, iri("urn:x:Graph"));
            assert!(suggestions[0].score > 0.8);
            assert!(suggestions.len() <= 3);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        reason(&out, "urn:x:Invented"),
        WithholdReason::Ungrounded { .. }
    ));

    // The patch is bound to the note's digest, applies, and reads back.
    let patch = out.patch.as_ref().unwrap();
    assert_eq!(patch.expected, kg.digest());
    let applied = kg.apply(patch).unwrap();
    let after = &applied.document;
    assert_eq!(
        after.value("is-a"),
        Some(&Value::strings(["[[Thing]]", "[[Graph]]"]))
    );
    // Written typed, as the RDF reads it.
    assert_eq!(after.value("population"), Some(&Value::Int(1200)));
    // Untouched keys keep their bytes.
    assert!(after.source().contains("aliases:\n- KG\n"));
    assert_eq!(after.body(), kg.body());
    // Once applied, the same patch is refused: no double write.
    assert!(after.apply(patch).is_err());
    assert!(matches!(
        ingest.accept(after, &req, &responses),
        Err(IngestError::StaleRevision { .. })
    ));

    // Determinism and the wire format.
    let again = ingest.accept(kg, &req, &responses).unwrap();
    assert_eq!(again, out);
    let json = serde_json::to_string(&out).unwrap();
    let back: Candidates = serde_json::from_str(&json).unwrap();
    assert_eq!(back, out);
}

#[test]
fn lexical_tiers_ground_initials_and_refuse_distinguishing_marks() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, {
        let mut o = Options::default();
        o.min_chunk_chars = 1;
        o
    });
    let lab = v.doc("Lab");
    let req = ingest.plan(&DocId::new("Lab.md"), lab).unwrap();
    assert_eq!(req.chunks.len(), 2);
    assert_eq!(req.chunks[0].heading_path, vec!["People".to_owned()]);

    let responses = vec![
        ModelResponse::new(
            0,
            vec![
                respond("part-of", "Baranov D.", "Run by Baranov D."),
                respond("founded", "2020", "since 2020"),
            ],
        ),
        ModelResponse::new(
            1,
            vec![
                respond("part-of", "Company U", "part of Company U"),
                respond("part-of", "Lab", "The lab"),
                respond("part-of", "Duplicate", "The lab"),
                respond("part-of", "Graph", "The lab"),
            ],
        ),
    ];
    let out = ingest.accept(lab, &req, &responses).unwrap();

    assert_eq!(out.accepted.len(), 1);
    assert_eq!(out.accepted[0].written, "[[Dmitry Baranov]]");
    assert_eq!(out.accepted[0].tier, Some(AliasTier::Initials));

    assert!(matches!(
        reason(&out, "2020"),
        WithholdReason::BadLiteral { .. }
    ));
    assert_eq!(
        reason(&out, "Company U"),
        &WithholdReason::InitialsConflict {
            with: iri("urn:x:CS")
        }
    );
    assert_eq!(reason(&out, "Lab"), &WithholdReason::SelfReference);
    assert_eq!(
        reason(&out, "Duplicate"),
        &WithholdReason::Ambiguous {
            candidates: vec![iri("urn:x:Dup")]
        }
    );
    assert_eq!(
        reason(&out, "Graph"),
        &WithholdReason::KindMismatch {
            expected: Kind::Individual,
            found: Kind::Class
        }
    );
    // Every refusal that found its quote carries the span.
    for w in &out.withheld {
        let ev = w.evidence.as_ref().unwrap();
        assert_eq!(ev.span.slice(lab.source()), ev.quote);
    }
}

#[test]
fn values_from_two_chunks_merge_their_evidence() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, {
        let mut o = Options::default();
        o.min_chunk_chars = 1;
        o
    });
    let lab = v.doc("Lab");
    let req = ingest.plan(&DocId::new("Lab.md"), lab).unwrap();
    let responses = vec![
        ModelResponse::new(
            1,
            vec![respond("part-of", "Baranov D", "Baranov D. founded it")],
        ),
        ModelResponse::new(
            0,
            vec![respond("part-of", "dmitry baranov", "Run by Baranov D.")],
        ),
    ];
    let out = ingest.accept(lab, &req, &responses).unwrap();
    assert_eq!(out.accepted.len(), 1);
    let a = &out.accepted[0];
    assert_eq!(a.written, "[[Dmitry Baranov]]");
    // Chunk order, not response order.
    assert_eq!(
        a.evidence.iter().map(|e| e.chunk).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        out.patch.as_ref().unwrap().ops.len(),
        1,
        "one op per key, not per response"
    );
}

#[test]
fn conflicting_single_values_are_refused_not_overwritten() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let g = v.doc("Graph");
    let req = ingest.plan(&DocId::new("Graph.md"), g).unwrap();
    let out = ingest
        .accept(
            g,
            &req,
            &[ModelResponse::new(
                0,
                vec![respond("founded", "2002-02-02", "Founded on 2002-02-02")],
            )],
        )
        .unwrap();
    assert!(out.patch.is_none());
    assert_eq!(
        reason(&out, "2002-02-02"),
        &WithholdReason::ConflictingValue {
            existing: "2001-01-01".into()
        }
    );
}

#[test]
fn tampered_requests_are_refused() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let kg = v.doc("Knowledge graph");
    let req = ingest.plan(&DocId::new("Knowledge graph.md"), kg).unwrap();
    let ok = vec![ModelResponse::new(
        0,
        vec![respond("is-a", "graph", "is a directed graph")],
    )];

    // A key the profile does not grant: here the note's identity key.
    let mut keys = req.clone();
    let mut extra = keys.keys[0].clone();
    extra.key = "iri".into();
    extra.predicate = iri("urn:x:iri");
    extra.form = PredicateForm::Annotation;
    extra.value = ValueKind::Literal {
        datatype: iri(&format!("{XSD}string")),
    };
    extra.range = None;
    extra.present = vec![];
    keys.keys.push(extra);
    assert_eq!(
        ingest.accept(kg, &keys, &ok),
        Err(IngestError::RequestMismatch { field: "keys" })
    );
    // A span outside the note: refused, not a panic.
    let mut chunks = req.clone();
    chunks.chunks[0].span = Span::new(10_000, 10_010);
    assert_eq!(
        ingest.accept(kg, &chunks, &ok),
        Err(IngestError::RequestMismatch { field: "chunks" })
    );
    // Another entity's identity.
    let mut note = req.clone();
    note.note.iri = iri("urn:x:Graph");
    assert_eq!(
        ingest.accept(kg, &note, &ok),
        Err(IngestError::RequestMismatch { field: "note" })
    );
    // The untouched request still works.
    assert_eq!(ingest.accept(kg, &req, &ok).unwrap().accepted.len(), 1);
}

#[test]
fn evidence_must_mention_the_value() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let kg = v.doc("Knowledge graph");
    let req = ingest.plan(&DocId::new("Knowledge graph.md"), kg).unwrap();
    let out = ingest
        .accept(
            kg,
            &req,
            &[ModelResponse::new(
                0,
                vec![
                    // A one-letter quote is in the chunk but says nothing.
                    respond("population", "999", "a"),
                    // A real quote that does not name the entity.
                    respond("is-a", "Graph", "It holds"),
                    // The entity named by its label though the quote has
                    // only its note name: fine.
                    respond("is-a", "graph", "a directed graph"),
                ],
            )],
        )
        .unwrap();
    assert_eq!(reason(&out, "999"), &WithholdReason::EvidenceDoesNotMention);
    assert_eq!(
        reason(&out, "Graph"),
        &WithholdReason::EvidenceDoesNotMention
    );
    assert_eq!(out.accepted.len(), 1);
    assert_eq!(out.accepted[0].written, "[[Graph]]");
}

#[test]
fn ambiguous_note_names_get_path_links_and_a_rerun_adds_nothing() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let id = DocId::new("Site.md");
    let site = v.doc("Site");
    let req = ingest.plan(&id, site).unwrap();
    let responses = vec![ModelResponse::new(
        0,
        vec![
            respond("part-of", "Alpha", "Hosted by Alpha"),
            respond("part-of", "Beta", "and Beta"),
        ],
    )];
    let out = ingest.accept(site, &req, &responses).unwrap();

    // Two entities, two candidates, neither written as the ambiguous
    // bare [[Twin]].
    let written: Vec<(&str, Option<&str>)> = out
        .accepted
        .iter()
        .map(|a| (a.written.as_str(), a.grounded.as_ref().map(Iri::as_str)))
        .collect();
    assert_eq!(
        written,
        [
            ("[[a/Twin]]", Some("urn:x:A")),
            ("[[b/Twin]]", Some("urn:x:B"))
        ]
    );
    // The null `part-of:` is treated as absent: no empty item is kept.
    let after = site.apply(out.patch.as_ref().unwrap()).unwrap().document;
    assert_eq!(
        after.value("part-of"),
        Some(&Value::strings(["[[a/Twin]]", "[[b/Twin]]"]))
    );
    // Re-planning on the patched note and accepting the same responses
    // writes nothing more.
    let req2 = ingest.plan(&id, &after).unwrap();
    let again = ingest.accept(&after, &req2, &responses).unwrap();
    assert!(again.patch.is_none());
    assert_eq!(reason(&again, "Alpha"), &WithholdReason::AlreadyPresent);
    assert_eq!(reason(&again, "Beta"), &WithholdReason::AlreadyPresent);
}

#[test]
fn null_keys_extend_and_competing_single_values_are_withheld() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let site = v.doc("Site");
    let req = ingest.plan(&DocId::new("Site.md"), site).unwrap();
    let out = ingest
        .accept(
            site,
            &req,
            &[ModelResponse::new(
                0,
                vec![
                    respond("founded", "2021-05-05", "Opened 2021-05-05"),
                    respond("weight", "3.5", "Weighs 3.5"),
                    respond("weight", "4.5", "or 4.5"),
                    respond("weight", "3,5", "Weighs 3.5"),
                ],
            )],
        )
        .unwrap();
    // `founded:` held nothing, so the date is written, not refused as a
    // conflict with an empty value.
    assert_eq!(out.accepted.len(), 1);
    assert_eq!(out.accepted[0].written, "2021-05-05");
    let after = site.apply(out.patch.as_ref().unwrap()).unwrap().document;
    assert_eq!(after.value("founded"), Some(&Value::string("2021-05-05")));
    assert_eq!(after.value("weight"), None);
    // Two values for one single-valued key: both withheld, naming each other.
    assert_eq!(
        reason(&out, "3.5"),
        &WithholdReason::CompetingValues {
            others: vec!["4.5".into()]
        }
    );
    assert_eq!(
        reason(&out, "4.5"),
        &WithholdReason::CompetingValues {
            others: vec!["3.5".into()]
        }
    );
    // A decimal comma is not a thousands separator.
    assert!(matches!(
        reason(&out, "3,5"),
        WithholdReason::BadLiteral { .. }
    ));
}

#[test]
fn request_integrity_is_checked() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let kg = v.doc("Knowledge graph");
    let req = ingest.plan(&DocId::new("Knowledge graph.md"), kg).unwrap();

    let mut other = req.clone();
    other.contract = "ontomark-ingest/facts/0".into();
    assert!(matches!(
        ingest.accept(kg, &other, &[]),
        Err(IngestError::ContractMismatch { .. })
    ));
    assert_eq!(
        ingest.accept(kg, &req, &[ModelResponse::new(7, vec![])]),
        Err(IngestError::UnknownChunk { chunk: 7, count: 1 })
    );
    // Nothing proposed: no patch, one informational diagnostic.
    let out = ingest
        .accept(kg, &req, &[ModelResponse::new(0, vec![])])
        .unwrap();
    assert!(out.patch.is_none());
    assert!(out.accepted.is_empty());
    assert_eq!(out.diagnostics[0].code, "empty_response");
}

#[test]
fn vocabulary_grounding_is_inspectable() {
    let p = profile();
    let v = Vault::new();
    let m = v.model(&p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let vocab = ingest.vocabulary();
    assert_eq!(vocab.terms().len(), m.entities.len());
    assert_eq!(
        vocab.term(&iri("urn:x:KG")).unwrap().note_name(),
        "Knowledge graph"
    );
    let t = &Options::default().lexical;
    assert_eq!(
        vocab.ground(&p, &m, "kg", t, 3),
        Grounding::Resolved {
            iri: iri("urn:x:KG"),
            tier: AliasTier::Exact
        }
    );
    assert_eq!(
        vocab.ground(&p, &m, "KnowledgeGraph", t, 3),
        Grounding::Resolved {
            iri: iri("urn:x:KG"),
            tier: AliasTier::Exact
        }
    );
    assert!(matches!(
        vocab.ground(&p, &m, "", t, 3),
        Grounding::Ungrounded { ref suggestions } if suggestions.is_empty()
    ));
    // A name that is one of a term's own names is never that term's
    // initials conflict, even when its other names differ by an initial.
    assert_eq!(
        vocab.ground(&p, &m, "company y", t, 3),
        Grounding::Resolved {
            iri: iri("urn:x:CX"),
            tier: AliasTier::Exact
        }
    );
    // A name differing from a term only by an initial is still a conflict.
    assert_eq!(
        vocab.ground(&p, &m, "Company Z", t, 3),
        Grounding::InitialsConflict {
            with: iri("urn:x:CS")
        }
    );
}
