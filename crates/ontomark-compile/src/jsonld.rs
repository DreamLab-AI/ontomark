//! JSON-LD 1.1 serialisation of an asserted [`Graph`].
//!
//! [`to_jsonld`] writes the same triples [`serialise`](crate::build::turtle::serialise)
//! writes as Turtle, as an expanded-style JSON-LD 1.1 document:
//!
//! ```json
//! { "@context": { "...": "..." },
//!   "@graph": [ { "@id": "https://example.org/a",
//!                 "@type": ["http://www.w3.org/2002/07/owl#Class"],
//!                 "http://www.w3.org/2000/01/rdf-schema#label":
//!                     [ { "@value": "A", "@language": "en" } ] } ] }
//! ```
//!
//! The document is deliberately *not* compacted. Every property, `@id` and
//! datatype is a full IRI, so reading it needs no context at all and a
//! processor cannot mis-expand a term; the supplied context is embedded for
//! consumers that want to compact or frame the data themselves.
//!
//! # Mapping
//!
//! | RDF | JSON-LD |
//! |---|---|
//! | subject | one node object per subject, `@id` an IRI or a `_:label` blank node |
//! | `rdf:type` with an IRI object | `@type` (a sorted array of IRIs) |
//! | any other IRI object | `{"@id": …}` |
//! | blank-node object | `{"@id": "_:label"}` |
//! | plain literal | `{"@value": …}` |
//! | language-tagged literal | `{"@value": …, "@language": …}` |
//! | typed literal | `{"@value": <lexical form as a string>, "@type": …}` |
//! | well-formed `rdf:first`/`rdf:rest` chain | `{"@list": [...]}` |
//!
//! Typed literals keep their **lexical form** as a JSON string: a native JSON
//! number or boolean would be re-canonicalised by a processor and break
//! round-tripping (`"0.80"^^xsd:float` must not become `0.8`).
//!
//! A chain is collapsed to `@list` only when it is exactly a list: the head
//! blank node is the object of one triple, every cell is a blank node carrying
//! `rdf:first` and `rdf:rest` and nothing else, every cell is referenced once,
//! and the chain ends in `rdf:nil` without a cycle. Anything else is written as
//! ordinary nodes, which is what the Turtle holds.
//!
//! Output is deterministic: nodes sorted by subject, properties by IRI, values
//! by the graph's own term order.
//!
//! # Limits
//!
//! An IRI that starts with `x:` where `x` is also a prefix the embedded context
//! defines (for example the literal IRI `owl:Thing`) would be read as a compact
//! IRI by a processor. The relocated build never emits such IRIs (it expands
//! them first); [`to_jsonld`] does not rewrite them.
//!
//! # Example
//!
//! ```
//! use ontomark_compile::build::turtle::{Graph, Term};
//! use ontomark_compile::jsonld::to_jsonld;
//! use serde_json::json;
//!
//! let mut g = Graph::new();
//! g.add_iri(
//!     "https://example.org/a",
//!     "http://www.w3.org/2000/01/rdf-schema#label",
//!     Term::lang("A", "en"),
//! );
//! let doc = to_jsonld(&g, &json!({"@context": {"ex": "https://example.org/"}}));
//! assert_eq!(doc["@context"]["ex"], "https://example.org/");
//! assert_eq!(doc["@graph"][0]["@id"], "https://example.org/a");
//! assert_eq!(
//!     doc["@graph"][0]["http://www.w3.org/2000/01/rdf-schema#label"][0],
//!     json!({"@value": "A", "@language": "en"})
//! );
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{json, Map, Value};

use crate::build::turtle::{Graph, Subject, Term};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// A recognised `rdf:first`/`rdf:rest` chain.
struct ListInfo {
    /// The members, in order.
    items: Vec<Term>,
    /// Every cell blank node of the chain (they are not emitted as nodes).
    cells: Vec<String>,
}

/// One incoming edge of a blank node: the predicate that reaches it.
type InEdges<'a> = HashMap<&'a str, Vec<&'a str>>;

/// The predicates of the triples that have each blank node as their object.
fn blank_in_edges(graph: &Graph) -> InEdges<'_> {
    let mut edges: InEdges<'_> = HashMap::new();
    for (_, predicate, object) in graph.iter() {
        if let Term::Blank(label) = object {
            edges.entry(label.as_str()).or_default().push(predicate);
        }
    }
    edges
}

/// Index the graph's triples by blank-node subject.
fn blank_subjects(graph: &Graph) -> BTreeMap<&str, Vec<(&str, &Term)>> {
    let mut by_subject: BTreeMap<&str, Vec<(&str, &Term)>> = BTreeMap::new();
    for (subject, predicate, object) in graph.iter() {
        if let Subject::Blank(label) = subject {
            by_subject
                .entry(label.as_str())
                .or_default()
                .push((predicate, object));
        }
    }
    by_subject
}

/// A blank node that is nothing but one list cell, referenced exactly once.
fn is_cell(label: &str, cells: &BTreeMap<&str, Vec<(&str, &Term)>>, edges: &InEdges<'_>) -> bool {
    edges.get(label).map(Vec::len) == Some(1)
        && cells.get(label).is_some_and(|triples| {
            triples.len() == 2
                && triples.iter().any(|(p, _)| *p == RDF_FIRST)
                && triples.iter().any(|(p, _)| *p == RDF_REST)
        })
}

/// Walk the chain starting at `head`; `None` unless it is exactly a list.
fn read_list(
    head: &str,
    cells: &BTreeMap<&str, Vec<(&str, &Term)>>,
    edges: &InEdges<'_>,
) -> Option<ListInfo> {
    let mut items = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut order = Vec::new();
    let mut current = head;
    loop {
        if !seen.insert(current) || !is_cell(current, cells, edges) {
            return None; // a cycle, a shared cell or a cell with extra properties
        }
        let triples = &cells[current];
        let first = triples.iter().find(|(p, _)| *p == RDF_FIRST)?.1;
        let rest = triples.iter().find(|(p, _)| *p == RDF_REST)?.1;
        items.push(first.clone());
        order.push(current.to_owned());
        match rest {
            Term::Iri(iri) if iri == RDF_NIL => break,
            Term::Blank(next) => current = next.as_str(),
            _ => return None,
        }
    }
    Some(ListInfo {
        items,
        cells: order,
    })
}

/// The JSON-LD value object or node reference for one RDF term.
fn term_value(term: &Term, lists: &HashMap<String, ListInfo>) -> Value {
    match term {
        Term::Iri(iri) => json!({ "@id": iri }),
        Term::Blank(label) => match lists.get(label) {
            Some(list) => {
                let items: Vec<Value> = list.items.iter().map(|t| term_value(t, lists)).collect();
                json!({ "@list": items })
            }
            None => json!({ "@id": format!("_:{label}") }),
        },
        Term::Literal {
            value,
            lang,
            datatype,
        } => {
            let mut object = Map::new();
            object.insert("@value".into(), Value::String(value.clone()));
            if let Some(lang) = lang {
                object.insert("@language".into(), Value::String(lang.clone()));
            } else if let Some(datatype) = datatype.as_ref().filter(|d| d.as_str() != XSD_STRING) {
                object.insert("@type".into(), Value::String(datatype.clone()));
            }
            Value::Object(object)
        }
    }
}

/// Serialise `graph` as a JSON-LD 1.1 document.
///
/// `context` is the `@context` to embed: either a bare context object or a
/// document of the form `{"@context": …}`, which is what
/// [`build::okf::context`](crate::build::okf::context) returns. A
/// `null` context embeds an empty one.
///
/// The result is `{"@context": …, "@graph": [...]}`; see the [module
/// documentation](self) for the mapping and the guarantees.
#[must_use]
pub fn to_jsonld(graph: &Graph, context: &Value) -> Value {
    let context = match context {
        Value::Object(map) if map.contains_key("@context") => map["@context"].clone(),
        Value::Null => json!({}),
        other => other.clone(),
    };

    // Lists first: a head is a cell whose single in-edge is not an `rdf:rest`.
    let edges = blank_in_edges(graph);
    let cells = blank_subjects(graph);
    let mut lists: HashMap<String, ListInfo> = HashMap::new();
    for &label in cells.keys() {
        let is_head = is_cell(label, &cells, &edges) && edges[label][0] != RDF_REST;
        if is_head {
            if let Some(list) = read_list(label, &cells, &edges) {
                lists.insert(label.to_owned(), list);
            }
        }
    }
    // Interior cells are written inside their head's `@list`, not as nodes.
    let hidden: BTreeSet<&str> = lists
        .values()
        .flat_map(|list| list.cells.iter().map(String::as_str))
        .collect();

    // Node objects, one per non-hidden subject.
    let mut nodes: BTreeMap<(u8, String), BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    let mut types: BTreeMap<(u8, String), BTreeSet<String>> = BTreeMap::new();
    for (subject, predicate, object) in graph.iter() {
        let key = match subject {
            Subject::Iri(iri) => (0u8, iri.clone()),
            Subject::Blank(label) => {
                if hidden.contains(label.as_str()) {
                    continue;
                }
                (1u8, format!("_:{label}"))
            }
        };
        if predicate == RDF_TYPE {
            if let Term::Iri(class) = object {
                types.entry(key.clone()).or_default().insert(class.clone());
                nodes.entry(key).or_default();
                continue;
            }
        }
        nodes
            .entry(key)
            .or_default()
            .entry(predicate.to_owned())
            .or_default()
            .push(term_value(object, &lists));
    }

    let graph_array: Vec<Value> = nodes
        .into_iter()
        .map(|(key, properties)| {
            let mut node = Map::new();
            node.insert("@id".into(), Value::String(key.1.clone()));
            if let Some(classes) = types.get(&key) {
                node.insert(
                    "@type".into(),
                    Value::Array(classes.iter().cloned().map(Value::String).collect()),
                );
            }
            for (predicate, values) in properties {
                node.insert(predicate, Value::Array(values));
            }
            Value::Object(node)
        })
        .collect();

    let mut document = Map::new();
    document.insert("@context".into(), context);
    document.insert("@graph".into(), Value::Array(graph_array));
    Value::Object(document)
}

/// [`to_jsonld`], pretty-printed with a trailing newline.
///
/// # Panics
/// Never in practice: a [`Value`] built from strings always serialises.
#[must_use]
pub fn to_jsonld_string(graph: &Graph, context: &Value) -> String {
    let mut out = serde_json::to_string_pretty(&to_jsonld(graph, context))
        .expect("a JSON value built from strings serialises");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EX: &str = "https://example.org/";

    fn ex(local: &str) -> String {
        format!("{EX}{local}")
    }

    #[test]
    fn a_well_formed_chain_becomes_a_list() {
        let mut g = Graph::new();
        let (c1, c2) = (g.fresh_blank(), g.fresh_blank());
        g.add_iri(&ex("s"), &ex("p"), Term::Blank(c1.clone()));
        g.add(Subject::Blank(c1.clone()), RDF_FIRST, Term::plain("a"));
        g.add(Subject::Blank(c1), RDF_REST, Term::Blank(c2.clone()));
        g.add(Subject::Blank(c2.clone()), RDF_FIRST, Term::plain("b"));
        g.add(Subject::Blank(c2), RDF_REST, Term::Iri(RDF_NIL.to_owned()));
        let doc = to_jsonld(&g, &Value::Null);
        let graph = doc["@graph"].as_array().unwrap();
        assert_eq!(graph.len(), 1, "cells are absorbed: {doc}");
        assert_eq!(
            graph[0][ex("p")][0],
            json!({"@list": [{"@value": "a"}, {"@value": "b"}]})
        );
    }

    #[test]
    fn a_shared_or_cyclic_chain_stays_nodes() {
        let mut g = Graph::new();
        let c = g.fresh_blank();
        g.add_iri(&ex("s"), &ex("p"), Term::Blank(c.clone()));
        g.add_iri(&ex("t"), &ex("p"), Term::Blank(c.clone()));
        g.add(Subject::Blank(c.clone()), RDF_FIRST, Term::plain("a"));
        g.add(Subject::Blank(c), RDF_REST, Term::Iri(RDF_NIL.to_owned()));
        let doc = to_jsonld(&g, &Value::Null);
        assert!(
            doc.to_string().contains("\"@id\":\"_:r1\""),
            "shared cell is a node: {doc}"
        );
        assert!(!doc.to_string().contains("@list"));

        let mut cyc = Graph::new();
        let (a, b) = (cyc.fresh_blank(), cyc.fresh_blank());
        cyc.add_iri(&ex("s"), &ex("p"), Term::Blank(a.clone()));
        cyc.add(Subject::Blank(a.clone()), RDF_FIRST, Term::plain("x"));
        cyc.add(Subject::Blank(a.clone()), RDF_REST, Term::Blank(b.clone()));
        cyc.add(Subject::Blank(b.clone()), RDF_FIRST, Term::plain("y"));
        cyc.add(Subject::Blank(b), RDF_REST, Term::Blank(a));
        assert!(!to_jsonld(&cyc, &Value::Null).to_string().contains("@list"));
    }

    #[test]
    fn a_cell_with_extra_properties_is_not_a_list() {
        let mut g = Graph::new();
        let c = g.fresh_blank();
        g.add_iri(&ex("s"), &ex("p"), Term::Blank(c.clone()));
        g.add(Subject::Blank(c.clone()), RDF_FIRST, Term::plain("a"));
        g.add(
            Subject::Blank(c.clone()),
            RDF_REST,
            Term::Iri(RDF_NIL.to_owned()),
        );
        g.add(Subject::Blank(c), ex("extra"), Term::plain("z"));
        assert!(!to_jsonld(&g, &Value::Null).to_string().contains("@list"));
    }

    #[test]
    fn typed_literals_keep_their_lexical_form() {
        let mut g = Graph::new();
        g.add_iri(
            &ex("s"),
            &ex("p"),
            Term::typed("0.80", "http://www.w3.org/2001/XMLSchema#float"),
        );
        g.add_iri(
            &ex("s"),
            &ex("q"),
            Term::typed("x", "http://www.w3.org/2001/XMLSchema#string"),
        );
        let doc = to_jsonld(&g, &Value::Null);
        assert_eq!(
            doc["@graph"][0][ex("p")][0],
            json!({"@value": "0.80", "@type": "http://www.w3.org/2001/XMLSchema#float"})
        );
        assert_eq!(doc["@graph"][0][ex("q")][0], json!({"@value": "x"}));
    }

    #[test]
    fn rdf_type_with_an_iri_object_is_at_type_and_with_a_literal_is_not() {
        let mut g = Graph::new();
        g.add_iri(&ex("s"), RDF_TYPE, Term::Iri(ex("B")));
        g.add_iri(&ex("s"), RDF_TYPE, Term::Iri(ex("A")));
        g.add_iri(&ex("s"), RDF_TYPE, Term::plain("odd"));
        let doc = to_jsonld(&g, &Value::Null);
        assert_eq!(doc["@graph"][0]["@type"], json!([ex("A"), ex("B")]));
        assert_eq!(doc["@graph"][0][RDF_TYPE][0], json!({"@value": "odd"}));
    }

    #[test]
    fn the_context_may_be_bare_or_wrapped() {
        let g = Graph::new();
        let wrapped = to_jsonld(&g, &json!({"@context": {"a": "b"}}));
        let bare = to_jsonld(&g, &json!({"a": "b"}));
        assert_eq!(wrapped, bare);
        assert_eq!(
            to_jsonld_string(&g, &Value::Null).chars().last(),
            Some('\n')
        );
    }
}
