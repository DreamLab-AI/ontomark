//! Grounding cost over a synthetic ~200-note vault: building the vocabulary
//! once, and grounding ~50 varied names (exact, case-variant, alias,
//! initials, misses). Throughput is reported per name.

use criterion::{black_box, Criterion, Throughput};
use ontomark_ingest::{LexicalThresholds, Vocabulary};
use ontomark_model::{DeclarativeProfile, DocId, Model, ProfileSpec};
use ontomark_parse::Document;

const FIRST: [&str; 10] = [
    "Alpha", "Baranov", "Cobalt", "Delta", "Ember", "Fjord", "Garnet", "Harbour", "Indigo",
    "Juniper",
];
const SECOND: [&str; 10] = [
    "Systems", "Research", "Labs", "Networks", "Dynamics", "Graph", "Index", "Works", "Group",
    "Core",
];

fn profile() -> DeclarativeProfile {
    let spec: ProfileSpec = serde_json::from_str(
        r##"{
        "id": "bench/1",
        "kinds": { "Class": "class", "NamedIndividual": "individual" },
        "predicates": {
            "is-a": { "iri": "http://www.w3.org/2000/01/rdf-schema#subClassOf", "form": "sub_class_of" }
        },
        "reserved": ["tags"]
    }"##,
    )
    .unwrap();
    DeclarativeProfile::new(spec).unwrap()
}

/// 200 typed notes. Names are 1-4 words; every tenth pair shares initials
/// (`A S`-style) and each note carries one alias.
fn docs() -> Vec<(DocId, Document)> {
    (0..200)
        .map(|i| {
            let a = FIRST[i % 10];
            let b = SECOND[(i / 10) % 10];
            let words = 1 + i % 4;
            let mut name = vec![a.to_owned(), b.to_owned()];
            if words > 2 {
                name.push(format!("Unit{}", i / 20));
            }
            if words > 3 {
                name.push("Division".to_owned());
            }
            name.truncate(words);
            let mut title = name.join(" ");
            if i >= 100 {
                title = format!("{title} {i}");
            }
            let alias = format!("{} {}", a.chars().next().unwrap(), b);
            let kind = if i % 3 == 0 {
                "Class"
            } else {
                "NamedIndividual"
            };
            let doc = Document::parse(format!(
                "---\ntype: {kind}\niri: urn:x:n{i}\naliases:\n- \"{alias} {i}\"\n---\nBody {i}.\n"
            ));
            (DocId::new(format!("{title}.md")), doc)
        })
        .collect()
}

fn names() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    // Exact labels.
    v.extend(["Alpha Systems", "Delta", "Cobalt Labs Unit1", "Fjord Index"].map(String::from));
    // Case and punctuation variants.
    v.extend(
        [
            "alpha systems",
            "DELTA",
            "cobalt  labs unit1",
            "FJORD-INDEX",
            "Ember Graph",
        ]
        .map(String::from),
    );
    // Aliases.
    v.extend((0..10).map(|i| format!("{} {} {i}", FIRST[i].chars().next().unwrap(), SECOND[0])));
    // Initials forms.
    v.extend(
        [
            "A S", "B R", "C L", "D N", "E D", "F G", "G I", "H W", "I G", "J C",
        ]
        .map(String::from),
    );
    // Near misses and misses.
    v.extend(
        [
            "Alpha Sistems",
            "Baranof Research",
            "Cobalt Lab",
            "Delta Network",
            "Embre Dynamics",
            "Quartz Holdings",
            "Zeppelin",
            "Nothing like this",
            "Harbor Works Group",
            "unrelated phrase here",
            "Juniper Core Division 99",
            "Totally Absent Entity",
            "Fjord",
            "Garnet Index 7",
            "Indigo Group Unit",
            "xyz",
        ]
        .map(String::from),
    );
    v
}

fn bench(c: &mut Criterion) {
    let profile = profile();
    let docs = docs();
    let model = Model::build(&profile, docs.iter().map(|(id, d)| (id.clone(), d)));
    let names = names();
    let thresholds = LexicalThresholds::default();

    c.bench_function("vocabulary_from_model_200_notes", |b| {
        b.iter(|| Vocabulary::from_model(black_box(&model)));
    });

    let vocab = Vocabulary::from_model(&model);
    let mut g = c.benchmark_group("ground");
    g.throughput(Throughput::Elements(names.len() as u64));
    g.bench_function(format!("{}_names", names.len()), |b| {
        b.iter(|| {
            for n in &names {
                black_box(vocab.ground(&profile, &model, black_box(n), &thresholds, 3));
            }
        });
    });
    g.finish();
}

#[allow(missing_docs)] // emitted by the criterion macro
mod harness {
    use super::bench;
    criterion::criterion_group!(benches, bench);
}

criterion::criterion_main!(harness::benches);
