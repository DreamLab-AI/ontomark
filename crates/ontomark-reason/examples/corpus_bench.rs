//! Classify a built corpus ontology and time it (feature `corpus-bench`).
//!
//! ```text
//! cargo run --release -p ontomark-reason --features corpus-bench \
//!     --example corpus_bench -- <ontology.ttl> [ontology-inferred.ttl] [all|relevant|both] [repeats]
//! ```
//!
//! Reads the asserted Turtle a `vault build` writes (`data/ontology.ttl`) and
//! maps it to axioms the way the estate reasoner's extractor does: declared
//! classes and properties, named `rdfs:subClassOf`, `owl:Restriction`
//! existentials in the superclass position, `owl:disjointWith`,
//! `owl:equivalentClass` (named, restriction or `owl:intersectionOf` list),
//! `rdfs:subPropertyOf` and `owl:TransitiveProperty`. When the inferred Turtle
//! is given, the classification's `inferred` pairs are compared with its
//! `rdfs:subClassOf` triples. Nothing from the corpus is written anywhere.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

use ontomark_reason::axiom::{Axiom, Characteristic, ClassExpression as C, Property};
use ontomark_reason::{classify_with, ExistentialPolicy, Options};
use oxttl::TurtleParser;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
const SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const SUBPROP: &str = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf";
const OWL: &str = "http://www.w3.org/2002/07/owl#";

/// A node: an IRI, or a blank node label prefixed with `_:`.
type Node = String;

fn triples(path: &str) -> Vec<(Node, String, Node)> {
    let file = BufReader::new(File::open(path).expect("open turtle"));
    let mut out = Vec::new();
    for t in TurtleParser::new().for_reader(file) {
        let t = t.expect("valid turtle");
        let s = match &t.subject {
            oxrdf::NamedOrBlankNode::NamedNode(n) => n.as_str().to_owned(),
            oxrdf::NamedOrBlankNode::BlankNode(b) => format!("_:{}", b.as_str()),
        };
        let o = match &t.object {
            oxrdf::Term::NamedNode(n) => n.as_str().to_owned(),
            oxrdf::Term::BlankNode(b) => format!("_:{}", b.as_str()),
            _ => continue,
        };
        out.push((s, t.predicate.as_str().to_owned(), o));
    }
    out
}

fn owl(local: &str) -> String {
    format!("{OWL}{local}")
}

type Index<'a> = BTreeMap<&'a str, Vec<(&'a str, &'a str)>>;

fn get<'a>(ix: &Index<'a>, s: &str, p: &str) -> Option<&'a str> {
    ix.get(s)
        .and_then(|v| v.iter().find(|(pp, _)| *pp == p).map(|(_, o)| *o))
}

fn restriction(ix: &Index<'_>, node: &str) -> Option<C> {
    let p = get(ix, node, &owl("onProperty"))?;
    let f = get(ix, node, &owl("someValuesFrom"))?;
    (!f.starts_with("_:")).then(|| C::some(p, C::class(f)))
}

fn operand(ix: &Index<'_>, node: &str) -> Option<C> {
    if node.starts_with("_:") {
        restriction(ix, node)
    } else {
        Some(C::class(node))
    }
}

fn list<'a>(ix: &Index<'a>, mut node: &'a str) -> Option<Vec<C>> {
    let mut items = Vec::new();
    while node != RDF_NIL {
        items.push(operand(ix, get(ix, node, RDF_FIRST)?)?);
        node = get(ix, node, RDF_REST)?;
    }
    Some(items)
}

fn axioms(path: &str) -> Vec<Axiom> {
    let ts = triples(path);
    let mut by_subject: Index<'_> = BTreeMap::new();
    for (s, p, o) in &ts {
        by_subject.entry(s).or_default().push((p, o));
    }
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for (s, p, o) in &ts {
        if s.starts_with("_:") {
            continue;
        }
        match p.as_str() {
            RDF_TYPE if *o == owl("Class") => out.push(Axiom::DeclareClass(s.clone())),
            RDF_TYPE if *o == owl("ObjectProperty") => out.push(Axiom::DeclareProperty(s.clone())),
            RDF_TYPE if *o == owl("TransitiveProperty") => out.push(Axiom::Characteristic {
                property: Property::named(s.clone()),
                characteristic: Characteristic::Transitive,
            }),
            SUBPROP if !o.starts_with("_:") => out.push(Axiom::SubPropertyOf {
                sub: Property::named(s.clone()),
                sup: Property::named(o.clone()),
            }),
            SUBCLASS => match operand(&by_subject, o) {
                Some(sup) => out.push(Axiom::SubClassOf {
                    sub: C::class(s.clone()),
                    sup,
                }),
                None => skipped += 1,
            },
            _ if *p == owl("disjointWith") && !o.starts_with("_:") => {
                out.push(Axiom::DisjointClasses(vec![
                    C::class(s.clone()),
                    C::class(o.clone()),
                ]));
            }
            _ if *p == owl("equivalentClass") => {
                let definiens = if o.starts_with("_:") {
                    get(&by_subject, o, &owl("intersectionOf"))
                        .and_then(|l| list(&by_subject, l))
                        .map(C::And)
                        .or_else(|| restriction(&by_subject, o))
                } else {
                    Some(C::class(o.clone()))
                };
                match definiens {
                    Some(d) => out.push(Axiom::EquivalentClasses(vec![C::class(s.clone()), d])),
                    None => skipped += 1,
                }
            }
            _ => {}
        }
    }
    eprintln!(
        "read {} triples -> {} axioms ({skipped} unreadable class axioms skipped)",
        ts.len(),
        out.len()
    );
    out
}

fn inferred_pairs(path: &str) -> BTreeSet<(String, String)> {
    triples(path)
        .into_iter()
        .filter(|(s, p, o)| p == SUBCLASS && !s.starts_with("_:") && !o.starts_with("_:"))
        .map(|(s, _, o)| (s, o))
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ttl = args
        .get(1)
        .expect("usage: corpus_bench <ontology.ttl> [inferred.ttl] [all|relevant|both] [repeats]");
    let inferred_ttl = args.get(2).filter(|a| a.as_str() != "-");
    let modes: Vec<ExistentialPolicy> = match args.get(3).map(String::as_str).unwrap_or("both") {
        "all" => vec![ExistentialPolicy::All],
        "relevant" => vec![ExistentialPolicy::Relevant],
        _ => vec![ExistentialPolicy::Relevant, ExistentialPolicy::All],
    };
    let repeats: usize = args.get(4).and_then(|r| r.parse().ok()).unwrap_or(5);

    let t = Instant::now();
    let axioms = axioms(ttl);
    eprintln!("parse+map: {:?}", t.elapsed());
    let kinds = axioms
        .iter()
        .fold(BTreeMap::<&str, usize>::new(), |mut m, a| {
            let k = match a {
                Axiom::DeclareClass(_) => "declare_class",
                Axiom::DeclareProperty(_) => "declare_property",
                Axiom::SubClassOf {
                    sup: C::Class(_), ..
                } => "sub_class_of(named)",
                Axiom::SubClassOf { .. } => "sub_class_of(existential)",
                Axiom::DisjointClasses(_) => "disjoint_classes",
                Axiom::EquivalentClasses(_) => "equivalent_classes",
                Axiom::SubPropertyOf { .. } => "sub_property_of",
                Axiom::Characteristic { .. } => "transitive",
                _ => "other",
            };
            *m.entry(k).or_default() += 1;
            m
        });
    eprintln!("axioms by kind: {kinds:?}");

    for mode in modes {
        let options = Options {
            existentials: mode,
            max_steps: None,
        };
        let mut times = Vec::new();
        let mut last = None;
        for _ in 0..repeats {
            let t = Instant::now();
            let c = classify_with(&axioms, options).expect("no budget");
            times.push(t.elapsed());
            last = Some(c);
        }
        times.sort();
        let c = last.expect("at least one repeat");
        println!(
            "mode={mode:?} classes={} subsumptions={} inferred={} unsatisfiable={} refusals={} existentials_dropped={} steps={} \
             classify min={:?} median={:?} max={:?} (n={repeats})",
            c.classes,
            c.subsumptions.len(),
            c.inferred.len(),
            c.unsatisfiable.len(),
            c.refusals.len(),
            c.existentials_dropped,
            c.steps,
            times[0],
            times[times.len() / 2],
            times[times.len() - 1],
        );
        if let Some(path) = inferred_ttl {
            let estate = inferred_pairs(path);
            let ours: BTreeSet<(String, String)> = c.inferred.iter().cloned().collect();
            let missing = estate.difference(&ours).count();
            let extra = ours.difference(&estate).count();
            println!(
                "  vs estate ontology-inferred.ttl: estate={} ours={} missing={missing} extra={extra}",
                estate.len(),
                ours.len()
            );
            for p in estate.difference(&ours).take(5) {
                println!("    missing {p:?}");
            }
            for p in ours.difference(&estate).take(5) {
                println!("    extra {p:?}");
            }
        }
    }

    {
        // Phase breakdown under the Relevant policy.
        let t = Instant::now();
        let report = ontomark_reason::check(&axioms);
        let t_check = t.elapsed();
        let el: Vec<whelk_el::ElAxiom> = report.accepted.into_iter().map(|(_, a)| a).collect();
        let t = Instant::now();
        let (el, _) = ontomark_reason::classify::apply_policy(el, ExistentialPolicy::Relevant);
        let t_policy = t.elapsed();
        let t = Instant::now();
        let translated = whelk_el::translate(&el);
        let t_translate = t.elapsed();
        let t = Instant::now();
        let state = whelk_el::assert(&translated);
        let t_assert = t.elapsed();
        let t = Instant::now();
        let pairs = state.named_subsumption_ids();
        let t_pairs = t.elapsed();
        let groups = whelk_el::components(&el);
        println!(
            "components (relevant input): {} (largest {} of {} axioms)",
            groups.len(),
            groups.iter().map(Vec::len).max().unwrap_or(0),
            el.len()
        );
        println!(
            "phases (relevant): check {t_check:?}, policy {t_policy:?}, translate {t_translate:?} -> {} axioms, assert {t_assert:?}, named_subsumption_ids {t_pairs:?} -> {} pairs",
            translated.len(),
            pairs.len()
        );
    }

    if std::env::var_os("BENCH_RAW_ALL").is_some() {
        let el: Vec<whelk_el::ElAxiom> = ontomark_reason::check(&axioms)
            .accepted
            .into_iter()
            .map(|(_, a)| a)
            .collect();
        let t = Instant::now();
        let seq = whelk_el::classify(&el);
        let seq_time = t.elapsed();
        let groups = whelk_el::components(&el);
        let t = Instant::now();
        let par = whelk_el::classify_parallel(&el);
        let par_time = t.elapsed();
        println!(
            "whelk-el raw (all existentials): sequential {seq_time:?}, parallel {par_time:?} over {} components (largest {} axioms), equal={}",
            groups.len(),
            groups.iter().map(Vec::len).max().unwrap_or(0),
            seq == par
        );
    }
}
