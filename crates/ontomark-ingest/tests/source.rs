//! External-source ingest (ADR-OM-010): evidence in a source, patch on a
//! target, counts, and the numeric-token guard end to end.

use ontomark_ingest::{
    AliasTier, Grounding, Ingest, IngestError, ModelResponse, Options, ProposedAssertion,
    SourceRef, SourceText, WithholdReason, CONTRACT, CONTRACT_SOURCE,
};
use ontomark_model::{DeclarativeProfile, DocId, Iri, Model, ProfileSpec};
use ontomark_parse::Document;

const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_str(&format!(
        r##"{{
        "id": "source-test/1",
        "kinds": {{ "Class": "class" }},
        "predicates": {{
            "is-a": {{ "iri": "http://www.w3.org/2000/01/rdf-schema#subClassOf", "form": "sub_class_of" }},
            "founded": {{ "iri": "urn:x:founded", "form": "data_assertion", "datatype": "{XSD}date" }}
        }}
    }}"##
    ))
    .unwrap();
    DeclarativeProfile::new(spec).unwrap()
}

fn page(front: &str, body: &str) -> Document {
    Document::parse(format!("---\n{front}\n---\n{body}"))
}

fn vault() -> Vec<(DocId, Document)> {
    let class = |name: &str, extra: &str, body: &str| {
        (
            DocId::new(format!("{name}.md")),
            page(
                &format!("type: Class\niri: urn:x:{}{extra}", name.replace(' ', "_")),
                body,
            ),
        )
    };
    vec![
        class("Graph", "", ""),
        class("Thing", "", ""),
        class(
            "Knowledge graph",
            "\nfounded: 2019-03-01",
            "Own body mentions Thing.\n",
        ),
        class("Standard", "", ""),
        class("ISO 9001", "", ""),
        class("ISO 14001", "", ""),
        class("Llama 3.1 8B", "", ""),
        class("Llama 3.1 70B", "", ""),
        class("Web 2.0", "", ""),
        class("Web 3.0", "", ""),
        class("IEEE 802.11", "", ""),
        class("IEEE 802.3", "", ""),
        class("Empty", "", ""),
    ]
}

fn model(docs: &[(DocId, Document)], p: &DeclarativeProfile) -> Model {
    Model::build(p, docs.iter().map(|(i, d)| (i.clone(), d)))
}

fn target(docs: &[(DocId, Document)]) -> (&DocId, &Document) {
    let (id, d) = docs
        .iter()
        .find(|(i, _)| i.name() == "Knowledge graph")
        .unwrap();
    (id, d)
}

fn respond(chunk: usize, items: &[(&str, &str, &str)]) -> ModelResponse {
    ModelResponse::new(
        chunk,
        items
            .iter()
            .map(|(k, v, e)| ProposedAssertion::new(*k, *v, *e))
            .collect(),
    )
}

/// A source with a leading `---` rule (not frontmatter), multibyte text
/// before the quote and two headed sections.
const SOURCE: &str = "---\nÉmile’s survey 𝔘\n---\n# Overview\nThe knowledge graph is a graph, per Dvořák.\n\n# History\nIt was founded on 2020-05-05.\n";

fn setup() -> (Vec<(DocId, Document)>, DeclarativeProfile) {
    (vault(), profile())
}

#[test]
fn evidence_is_found_in_the_source_and_the_patch_lands_on_the_target() {
    let (docs, p) = setup();
    let m = model(&docs, &p);
    let mut opts = Options::default();
    opts.min_chunk_chars = 1;
    let ingest = Ingest::new(&p, &m, opts);
    let (id, kg) = target(&docs);
    let source = SourceText::from_text("file:survey.md", SOURCE);

    let req = ingest.plan_source(id, kg, &source).unwrap();
    assert_eq!(req.contract, CONTRACT_SOURCE);
    assert_eq!(req.source.as_ref(), Some(&source.reference));
    assert_eq!(req.note.digest, kg.digest().hex());
    assert_eq!(req.chunks.len(), 3);
    // The leading rule is source text, not frontmatter: spans start at 0.
    assert_eq!(req.chunks[0].span.start, 0);
    for c in &req.chunks {
        assert_eq!(c.span.slice(&source.text), c.text);
    }

    let out = ingest
        .accept_source(
            kg,
            &source,
            &req,
            &[respond(
                1,
                &[
                    ("is-a", "graph", "knowledge graph is a graph"),
                    // Present in the target's own body but not in the source.
                    ("is-a", "Thing", "Own body mentions Thing."),
                ],
            )],
        )
        .unwrap();

    assert_eq!(out.accepted.len(), 1);
    let ev = &out.accepted[0].evidence[0];
    assert_eq!(ev.source.as_ref(), Some(&source.reference));
    assert_eq!(ev.span.slice(&source.text), ev.quote);
    assert_eq!(ev.quote, "knowledge graph is a graph");
    // UTF-16 offsets count the astral character as two units.
    let units: Vec<u16> = source.text.encode_utf16().collect();
    let quoted = String::from_utf16(&units[ev.utf16.start..ev.utf16.end]).unwrap();
    assert_eq!(quoted, ev.quote);
    assert_eq!(out.source.as_ref(), Some(&source.reference));
    assert!(matches!(
        out.withheld[0].reason,
        WithholdReason::EvidenceNotFound
    ));

    // The patch is the target's: bound to its digest, body untouched.
    assert_eq!(out.digest, kg.digest().hex());
    let patched = kg.apply(out.patch.as_ref().unwrap()).unwrap();
    assert!(patched.document.source().contains("[[Graph]]"));
    assert_eq!(patched.document.body(), kg.body());

    let counts = out.counts.as_ref().unwrap();
    assert_eq!(counts.names.get(&AliasTier::Exact), Some(&1));
    assert_eq!(counts.literals, 0);
    assert_eq!(counts.withheld.get("evidence_not_found"), Some(&1));
    assert_eq!(*counts, out.tally());
}

#[test]
fn a_source_that_disagrees_is_withheld_not_written() {
    let (docs, p) = setup();
    let m = model(&docs, &p);
    let mut opts = Options::default();
    opts.min_chunk_chars = 1;
    let ingest = Ingest::new(&p, &m, opts);
    let (id, kg) = target(&docs);
    let source = SourceText::from_text("file:survey.md", SOURCE);
    let req = ingest.plan_source(id, kg, &source).unwrap();
    let out = ingest
        .accept_source(
            kg,
            &source,
            &req,
            &[respond(
                2,
                &[("founded", "2020-05-05", "founded on 2020-05-05")],
            )],
        )
        .unwrap();
    assert!(out.patch.is_none(), "{:#?}", out.withheld);
    assert!(
        matches!(
            out.withheld[0].reason,
            WithholdReason::ConflictingValue { .. }
        ),
        "{:?}",
        out.withheld[0].reason
    );
    assert_eq!(
        out.counts.unwrap().withheld.get("conflicting_value"),
        Some(&1)
    );
}

#[test]
fn literals_are_counted_apart_from_names() {
    let (docs, p) = setup();
    let m = model(&docs, &p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let id = DocId::new("Empty.md");
    let empty = &docs.iter().find(|(i, _)| *i == id).unwrap().1;
    let source = SourceText::from_text("file:s.md", "Founded on 2021-01-02. It is a graph.\n");
    let req = ingest.plan_source(&id, empty, &source).unwrap();
    let out = ingest
        .accept_source(
            empty,
            &source,
            &req,
            &[respond(
                0,
                &[
                    ("founded", "2021-01-02", "Founded on 2021-01-02"),
                    ("is-a", "graph", "is a graph"),
                    ("is-a", "nonesuch", "is a graph"),
                ],
            )],
        )
        .unwrap();
    let c = out.counts.unwrap();
    assert_eq!(c.literals, 1);
    assert_eq!(c.names.values().sum::<usize>(), 1);
    assert_eq!(c.withheld.get("ungrounded"), Some(&1));
}

#[test]
fn source_and_note_requests_cannot_be_crossed() {
    let (docs, p) = setup();
    let m = model(&docs, &p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let (id, kg) = target(&docs);
    let source = SourceText::from_text("file:survey.md", SOURCE);
    let sreq = ingest.plan_source(id, kg, &source).unwrap();
    let nreq = ingest.plan(id, kg).unwrap();
    assert_eq!(nreq.contract, CONTRACT);
    assert!(nreq.source.is_none());

    assert!(matches!(
        ingest.accept(kg, &sreq, &[]),
        Err(IngestError::ContractMismatch { .. })
    ));
    assert!(matches!(
        ingest.accept_source(kg, &source, &nreq, &[]),
        Err(IngestError::ContractMismatch { .. })
    ));

    // Another source: same shape, different text.
    let other = SourceText::from_text("file:survey.md", "Something else entirely.\n");
    assert_eq!(
        ingest.accept_source(kg, &other, &sreq, &[]),
        Err(IngestError::RequestMismatch { field: "source" })
    );
    // Same reference, altered chunks.
    let mut forged = sreq.clone();
    forged.chunks[0].text.push('!');
    assert_eq!(
        ingest.accept_source(kg, &source, &forged, &[]),
        Err(IngestError::RequestMismatch { field: "chunks" })
    );
    // A contract-source request stripped of its source.
    let mut stripped = sreq.clone();
    stripped.source = None;
    assert_eq!(
        ingest.accept_source(kg, &source, &stripped, &[]),
        Err(IngestError::RequestMismatch { field: "source" })
    );
    // A declared version that is not the text's hash.
    let lying = SourceText::new(SourceRef::new("file:survey.md", "00"), SOURCE);
    assert!(matches!(
        ingest.plan_source(id, kg, &lying),
        Err(IngestError::SourceVersion { .. })
    ));
    // The target moved on.
    let moved = page("type: Class\niri: urn:x:Knowledge_graph", "edited\n");
    assert!(matches!(
        ingest.accept_source(&moved, &source, &sreq, &[]),
        Err(IngestError::StaleRevision { .. })
    ));
}

#[test]
fn a_note_only_ingest_serialises_without_the_new_fields() {
    let (docs, p) = setup();
    let m = model(&docs, &p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let (id, kg) = target(&docs);
    let req = ingest.plan(id, kg).unwrap();
    let out = ingest
        .accept(
            kg,
            &req,
            &[respond(0, &[("is-a", "Graph", "Own body mentions Thing.")])],
        )
        .unwrap();
    let json = serde_json::to_string(&(&req, &out)).unwrap();
    assert!(!json.contains("\"source\""), "{json}");
    assert!(!json.contains("\"counts\""), "{json}");
    assert!(out.counts.is_none());
}

#[test]
fn withhold_categories_are_the_serde_tags() {
    let i = |s: &str| Iri::parse(s).unwrap();
    let kind = ontomark_model::Kind::Class;
    let reasons = vec![
        WithholdReason::UnknownKey,
        WithholdReason::EvidenceNotFound,
        WithholdReason::EvidenceDoesNotMention,
        WithholdReason::Ungrounded {
            suggestions: vec![],
        },
        WithholdReason::Ambiguous { candidates: vec![] },
        WithholdReason::InitialsConflict { with: i("urn:x:a") },
        WithholdReason::KindMismatch {
            expected: kind,
            found: kind,
        },
        WithholdReason::SelfReference,
        WithholdReason::Unlinkable { tried: vec![] },
        WithholdReason::CompetingValues { others: vec![] },
        WithholdReason::BadLiteral {
            datatype: i(&format!("{XSD}date")),
            detail: String::new(),
        },
        WithholdReason::ConflictingValue {
            existing: String::new(),
        },
        WithholdReason::UnpatchableKey,
        WithholdReason::AlreadyPresent,
        WithholdReason::JudgedUnsupported,
    ];
    for r in reasons {
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["reason"], r.category(), "{r:?}");
    }
}

#[test]
fn numeric_siblings_do_not_ground_to_each_other() {
    let (docs, p) = setup();
    let m = model(&docs, &p);
    let ingest = Ingest::new(&p, &m, Options::default());
    let v = ingest.vocabulary();
    let t = Options::default().lexical;
    let ground = |name: &str| v.ground(&p, &m, name, &t, 3);
    let iri_of = |g: Grounding| match g {
        Grounding::Resolved { iri, tier } => Some((iri.as_str().to_owned(), tier)),
        _ => None,
    };

    // Names that are not in the vault but differ from one that is only in a
    // number never resolve to it.
    for name in [
        "ISO 27001",
        "Llama 3.1 405B",
        "Web 4.0",
        "IEEE 802.15",
        "Llama 3.2 8B",
        "ISO",
    ] {
        assert_eq!(iri_of(ground(name)), None, "{name}");
    }
    // Real terms still resolve, case-folded, as Exact.
    for (name, want) in [
        ("iso 9001", "urn:x:ISO_9001"),
        ("ISO 14001", "urn:x:ISO_14001"),
        ("llama 3.1 8b", "urn:x:Llama_3.1_8B"),
        ("Web 3.0", "urn:x:Web_3.0"),
        ("ieee 802.11", "urn:x:IEEE_802.11"),
    ] {
        assert_eq!(
            iri_of(ground(name)),
            Some((want.to_owned(), AliasTier::Exact)),
            "{name}"
        );
    }
    // A pair that differs in words only keeps its fuzzy grounding.
    assert_eq!(
        iri_of(ground("knowledge graphs")).map(|(i, _)| i),
        Some("urn:x:Knowledge_graph".to_owned())
    );
}
