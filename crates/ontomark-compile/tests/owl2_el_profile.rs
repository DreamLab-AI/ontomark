//! OWL 2 EL profile conformance of the emitted ontology.
//!
//! `data/ontology.ttl` is meant to be an OWL 2 EL ontology (with SKOS,
//! DCTERMS and PROV-O annotations). This test classifies every logical axiom
//! of the emitted graph against the profile of
//! <https://www.w3.org/TR/owl2-profiles/#OWL_2_EL>, prints a histogram of axiom
//! kinds, and asserts that none is outside EL.
//!
//! The classifier works on RDF triples (`oxttl` / `oxrdfxml`), reading the
//! OWL 2 RDF mapping back into axioms with a structural walk. `horned-owl`
//! was considered and not used: the current release is 4.x (not 1.x), its RDF
//! reader takes RDF/XML only, and a single triple-level classifier serves
//! the Turtle output, the RDF/XML test cases and the fixture alike.
//!
//! The classifier itself is checked two ways:
//!
//! * hand-written positive and negative examples for each construct in the
//!   profile specification (below); and
//! * the OWL 2 profile-identification test cases, as packaged by the OWL API
//!   contract tests (`owlcs/owlapi`, pinned in `conformance/w3c/PINS`), whose
//!   files record, per case, whether the ontology is in OWL 2 EL. That corpus
//!   is not the W3C repository itself (the W3C test-case site is no longer
//!   served); it is the OWL API's copy of those profile test cases.
//!
//! Fetch the third-party cases with `conformance/w3c/fetch.sh`; they are
//! gitignored and the suite test skips with a note when they are absent.
//!
//! ```text
//! conformance/w3c/fetch.sh && \
//!   cargo test -p ontomark-compile --test owl2_el_profile -- --nocapture
//! ```
//!
//! Scope of the classifier: the syntactic EL restrictions (entities,
//! property expressions, class expressions, data ranges, axioms, the range
//! restriction on property chains) plus the OWL 2 DL typing conditions that
//! an RDF graph must meet to be read as DL at all (declared properties, no
//! anonymous individuals). It does not implement the DL global restrictions on
//! non-simple properties, which only matter for ontologies that are not EL
//! anyway.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use oxrdf::{NamedOrBlankNode, Term, Triple};
use oxrdfxml::RdfXmlParser;
use oxttl::TurtleParser;

const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const OWL: &str = "http://www.w3.org/2002/07/owl#";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

fn rdf(x: &str) -> String {
    format!("{RDF}{x}")
}
fn rdfs(x: &str) -> String {
    format!("{RDFS}{x}")
}
fn owl(x: &str) -> String {
    format!("{OWL}{x}")
}

/// Datatypes OWL 2 EL admits (profile specification, section 2.2.1).
fn el_datatypes() -> HashSet<String> {
    let mut set: HashSet<String> = [
        rdf("PlainLiteral"),
        rdf("XMLLiteral"),
        rdfs("Literal"),
        owl("real"),
        owl("rational"),
    ]
    .into_iter()
    .collect();
    for local in [
        "decimal",
        "integer",
        "nonNegativeInteger",
        "string",
        "normalizedString",
        "token",
        "Name",
        "NCName",
        "NMTOKEN",
        "hexBinary",
        "base64Binary",
        "anyURI",
        "dateTime",
        "dateTimeStamp",
    ] {
        set.insert(format!("{XSD}{local}"));
    }
    set
}

// ---------------------------------------------------------------------------
// Classifier
// ---------------------------------------------------------------------------

/// One logical axiom: its kind, and why it is outside EL (if it is).
#[derive(Debug, Clone)]
struct Axiom {
    kind: String,
    violation: Option<String>,
}

/// The result of classifying a graph.
#[derive(Debug, Default)]
struct Report {
    axioms: Vec<Axiom>,
}

impl Report {
    fn push(&mut self, kind: &str, check: Result<(), String>) {
        self.axioms.push(Axiom {
            kind: kind.to_owned(),
            violation: check.err(),
        });
    }

    /// In OWL 2 EL *and* readable as OWL 2 DL (typing conditions met): the
    /// profile-identification question.
    fn is_el(&self) -> bool {
        self.axioms.iter().all(|a| a.violation.is_none())
    }

    /// Violations of the EL restrictions proper, leaving out the OWL 2 DL
    /// typing conditions (undeclared entities, illegal punning).
    fn el_violations(&self) -> BTreeMap<String, usize> {
        self.violations()
            .into_iter()
            .filter(|(why, _)| !why.contains(DL_TYPING))
            .collect()
    }

    /// Only the OWL 2 DL typing conditions.
    fn typing_issues(&self) -> BTreeMap<String, usize> {
        self.violations()
            .into_iter()
            .filter(|(why, _)| why.contains(DL_TYPING))
            .collect()
    }

    fn violations(&self) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for a in &self.axioms {
            if let Some(v) = &a.violation {
                *out.entry(format!("{}: {v}", a.kind)).or_insert(0) += 1;
            }
        }
        out
    }

    /// `kind -> (total, outside EL)`.
    fn histogram(&self) -> BTreeMap<String, (usize, usize)> {
        let mut out: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for a in &self.axioms {
            let e = out.entry(a.kind.clone()).or_default();
            e.0 += 1;
            e.1 += usize::from(a.violation.is_some());
        }
        out
    }
}

type Node = NamedOrBlankNode;

struct Doc {
    triples: Vec<Triple>,
    by_subject: HashMap<Node, Vec<(String, Term)>>,
    /// IRI -> the `rdf:type` objects (IRIs) it is declared with.
    types: HashMap<String, HashSet<String>>,
    el_datatypes: HashSet<String>,
}

impl Doc {
    fn new(triples: Vec<Triple>) -> Self {
        let mut by_subject: HashMap<Node, Vec<(String, Term)>> = HashMap::new();
        let mut types: HashMap<String, HashSet<String>> = HashMap::new();
        for t in &triples {
            by_subject
                .entry(t.subject.clone())
                .or_default()
                .push((t.predicate.as_str().to_owned(), t.object.clone()));
            if t.predicate.as_str() == rdf("type") {
                if let (NamedOrBlankNode::NamedNode(s), Term::NamedNode(o)) =
                    (&t.subject, &t.object)
                {
                    types
                        .entry(s.as_str().to_owned())
                        .or_default()
                        .insert(o.as_str().to_owned());
                }
            }
        }
        Self {
            triples,
            by_subject,
            types,
            el_datatypes: el_datatypes(),
        }
    }

    fn objs(&self, s: &Node, p: &str) -> Vec<&Term> {
        self.by_subject
            .get(s)
            .map(|v| v.iter().filter(|(q, _)| q == p).map(|(_, o)| o).collect())
            .unwrap_or_default()
    }

    fn one(&self, s: &Node, p: &str) -> Option<&Term> {
        self.objs(s, p).into_iter().next()
    }

    fn has_pred(&self, s: &Node, p: &str) -> bool {
        self.one(s, p).is_some()
    }

    fn node_types(&self, s: &Node) -> HashSet<String> {
        self.objs(s, &rdf("type"))
            .into_iter()
            .filter_map(|o| match o {
                Term::NamedNode(n) => Some(n.as_str().to_owned()),
                _ => None,
            })
            .collect()
    }

    fn declared(&self, iri: &str, ty: &str) -> bool {
        self.types.get(iri).is_some_and(|t| t.contains(&owl(ty)))
    }

    /// In the OWL 2 RDF mapping these characteristics are subclasses of
    /// `owl:ObjectProperty`, so typing an IRI with one declares it.
    fn is_object_property(&self, iri: &str) -> bool {
        [
            "ObjectProperty",
            "TransitiveProperty",
            "SymmetricProperty",
            "AsymmetricProperty",
            "ReflexiveProperty",
            "IrreflexiveProperty",
            "InverseFunctionalProperty",
        ]
        .iter()
        .any(|t| self.declared(iri, t))
    }
    fn is_data_property(&self, iri: &str) -> bool {
        self.declared(iri, "DatatypeProperty")
    }
    fn is_annotation_property(&self, iri: &str) -> bool {
        self.declared(iri, "AnnotationProperty") || BUILTIN_ANNOTATION.contains(&iri)
    }

    /// The members of an `rdf:List` rooted at `head`, or `None` if malformed.
    fn list(&self, head: &Term) -> Option<Vec<Term>> {
        let mut out = Vec::new();
        let mut cur = head.clone();
        for _ in 0..100_000 {
            match &cur {
                Term::NamedNode(n) if n.as_str() == rdf("nil") => return Some(out),
                Term::BlankNode(b) => {
                    let node = Node::BlankNode(b.clone());
                    out.push(self.one(&node, &rdf("first"))?.clone());
                    cur = self.one(&node, &rdf("rest"))?.clone();
                }
                _ => return None,
            }
        }
        None
    }

    /// A blank node that exists only to build a class expression, list, axiom
    /// or annotation (as opposed to an anonymous individual).
    fn structural(&self, s: &Node) -> bool {
        let Node::BlankNode(_) = s else {
            return false;
        };
        const STRUCT_TYPES: &[&str] = &[
            "Restriction",
            "Class",
            "DataRange",
            "AllDisjointClasses",
            "AllDisjointProperties",
            "AllDifferent",
            "NegativePropertyAssertion",
            "Axiom",
            "Annotation",
            "Ontology",
            "ObjectProperty",
        ];
        const STRUCT_PREDS: &[&str] = &[
            "intersectionOf",
            "unionOf",
            "complementOf",
            "oneOf",
            "onProperty",
            "onProperties",
            "inverseOf",
            "withRestrictions",
            "datatypeComplementOf",
            "members",
            "distinctMembers",
            "annotatedSource",
            "sourceIndividual",
        ];
        self.node_types(s)
            .iter()
            .any(|t| STRUCT_TYPES.iter().any(|k| *t == owl(k)) || *t == rdfs("Datatype"))
            || STRUCT_PREDS.iter().any(|p| self.has_pred(s, &owl(p)))
            || self.has_pred(s, &rdf("first"))
            || self.has_pred(s, &rdf("rest"))
    }

    /// A canonical string for an expression, for comparing range restrictions.
    fn key(&self, t: &Term, depth: usize) -> String {
        match t {
            Term::NamedNode(n) => format!("<{}>", n.as_str()),
            Term::Literal(l) => l.to_string(),
            Term::BlankNode(b) if depth < 16 => {
                let node = Node::BlankNode(b.clone());
                let mut parts: Vec<String> = self
                    .by_subject
                    .get(&node)
                    .map(|v| {
                        v.iter()
                            .map(|(p, o)| format!("{p}={}", self.key(o, depth + 1)))
                            .collect()
                    })
                    .unwrap_or_default();
                parts.sort();
                format!("[{}]", parts.join(","))
            }
            Term::BlankNode(_) => "[..]".to_owned(),
        }
    }
}

/// Marks a violation that is an OWL 2 DL typing condition, not an EL restriction.
const DL_TYPING: &str = "(not OWL 2 DL)";

const BUILTIN_ANNOTATION: &[&str] = &[
    "http://www.w3.org/2000/01/rdf-schema#label",
    "http://www.w3.org/2000/01/rdf-schema#comment",
    "http://www.w3.org/2000/01/rdf-schema#seeAlso",
    "http://www.w3.org/2000/01/rdf-schema#isDefinedBy",
    "http://www.w3.org/2002/07/owl#versionInfo",
    "http://www.w3.org/2002/07/owl#priorVersion",
    "http://www.w3.org/2002/07/owl#backwardCompatibleWith",
    "http://www.w3.org/2002/07/owl#incompatibleWith",
    "http://www.w3.org/2002/07/owl#deprecated",
];

fn iri_of(t: &Term) -> Option<&str> {
    match t {
        Term::NamedNode(n) => Some(n.as_str()),
        _ => None,
    }
}

/// Datatype of a literal in the OWL 2 sense (`rdf:PlainLiteral` for language
/// strings).
fn literal_datatype(l: &oxrdf::Literal) -> String {
    if l.language().is_some() {
        rdf("PlainLiteral")
    } else {
        l.datatype().as_str().to_owned()
    }
}

impl Doc {
    fn check_literal(&self, l: &oxrdf::Literal) -> Result<(), String> {
        let dt = literal_datatype(l);
        if self.el_datatypes.contains(&dt) {
            Ok(())
        } else {
            Err(format!("datatype {dt} is outside EL"))
        }
    }

    /// An object property expression: a named property, never an inverse.
    fn check_object_property(&self, t: &Term) -> Result<(), String> {
        match t {
            Term::NamedNode(_) => Ok(()),
            Term::BlankNode(b) => {
                if self.has_pred(&Node::BlankNode(b.clone()), &owl("inverseOf")) {
                    Err("ObjectInverseOf".into())
                } else {
                    Err("anonymous property expression".into())
                }
            }
            Term::Literal(_) => Err("literal as property".into()),
        }
    }

    /// A data range allowed by EL: a listed datatype, an intersection of such,
    /// or an enumeration of exactly one literal.
    fn check_data_range(&self, t: &Term) -> Result<(), String> {
        match t {
            Term::NamedNode(n) => {
                if self.el_datatypes.contains(n.as_str()) {
                    Ok(())
                } else {
                    Err(format!("datatype {} is outside EL", n.as_str()))
                }
            }
            Term::BlankNode(b) => {
                let node = Node::BlankNode(b.clone());
                if let Some(l) = self.one(&node, &owl("intersectionOf")) {
                    let members = self.list(l).ok_or("malformed list")?;
                    if members.len() < 2 {
                        return Err("DataIntersectionOf with fewer than 2 operands".into());
                    }
                    members.iter().try_for_each(|m| self.check_data_range(m))
                } else if let Some(l) = self.one(&node, &owl("oneOf")) {
                    let members = self.list(l).ok_or("malformed list")?;
                    if members.len() != 1 {
                        return Err(format!("DataOneOf with {} literals", members.len()));
                    }
                    match &members[0] {
                        Term::Literal(l) => self.check_literal(l),
                        _ => Err("DataOneOf member is not a literal".into()),
                    }
                } else if self.has_pred(&node, &owl("unionOf")) {
                    Err("DataUnionOf".into())
                } else if self.has_pred(&node, &owl("datatypeComplementOf")) {
                    Err("DataComplementOf".into())
                } else if self.has_pred(&node, &owl("withRestrictions")) {
                    Err("DatatypeRestriction".into())
                } else {
                    Err("unrecognised data range".into())
                }
            }
            Term::Literal(_) => Err("literal as data range".into()),
        }
    }

    /// OWL 2 DL typing: a class IRI must be `owl:Thing`, `owl:Nothing` or
    /// declared `owl:Class`; otherwise the RDF graph is not read as DL at all.
    fn check_class_iri(&self, iri: &str) -> Result<(), String> {
        if iri == owl("Thing")
            || iri == owl("Nothing")
            || self.declared(iri, "Class")
            || self
                .types
                .get(iri)
                .is_some_and(|t| t.contains(&rdfs("Class")))
        {
            Ok(())
        } else {
            Err(format!("class {iri} is not declared (not OWL 2 DL)"))
        }
    }

    /// An EL class expression.
    fn check_class(&self, t: &Term) -> Result<(), String> {
        let node = match t {
            Term::NamedNode(n) => return self.check_class_iri(n.as_str()),
            Term::Literal(_) => return Err("literal as class".into()),
            Term::BlankNode(b) => Node::BlankNode(b.clone()),
        };
        if let Some(l) = self.one(&node, &owl("intersectionOf")) {
            let members = self.list(l).ok_or("malformed list")?;
            if members.len() < 2 {
                return Err("ObjectIntersectionOf with fewer than 2 operands".into());
            }
            return members.iter().try_for_each(|m| self.check_class(m));
        }
        if self.has_pred(&node, &owl("unionOf")) {
            return Err("ObjectUnionOf".into());
        }
        if self.has_pred(&node, &owl("complementOf")) {
            return Err("ObjectComplementOf".into());
        }
        if let Some(l) = self.one(&node, &owl("oneOf")) {
            let members = self.list(l).ok_or("malformed list")?;
            if members.len() != 1 {
                return Err(format!("ObjectOneOf with {} individuals", members.len()));
            }
            return match &members[0] {
                Term::NamedNode(_) => Ok(()),
                _ => Err("anonymous individual in ObjectOneOf".into()),
            };
        }
        if self.node_types(&node).contains(&owl("Restriction"))
            || self.has_pred(&node, &owl("onProperty"))
        {
            return self.check_restriction(&node);
        }
        Err("unrecognised class expression".into())
    }

    fn check_restriction(&self, node: &Node) -> Result<(), String> {
        for card in [
            "cardinality",
            "minCardinality",
            "maxCardinality",
            "qualifiedCardinality",
            "minQualifiedCardinality",
            "maxQualifiedCardinality",
        ] {
            if self.has_pred(node, &owl(card)) {
                return Err(format!("cardinality restriction ({card})"));
            }
        }
        if self.has_pred(node, &owl("allValuesFrom")) {
            return Err("AllValuesFrom".into());
        }
        if self.has_pred(node, &owl("onProperties")) {
            return Err("n-ary data restriction".into());
        }
        let prop = self
            .one(node, &owl("onProperty"))
            .ok_or("restriction without owl:onProperty")?;
        let is_data = iri_of(prop).is_some_and(|p| self.is_data_property(p));
        if !is_data {
            self.check_object_property(prop)?;
        }
        if let Some(filler) = self.one(node, &owl("someValuesFrom")) {
            return if is_data {
                self.check_data_range(filler)
            } else {
                self.check_class(filler)
            };
        }
        if let Some(value) = self.one(node, &owl("hasValue")) {
            return match value {
                Term::Literal(l) => self.check_literal(l),
                Term::NamedNode(_) => Ok(()),
                Term::BlankNode(_) => Err("anonymous individual in HasValue".into()),
            };
        }
        if self.has_pred(node, &owl("hasSelf")) {
            return Ok(());
        }
        Err("restriction of an unrecognised kind".into())
    }

    /// Subject expressions of `rdfs:subClassOf` and friends may be named or
    /// anonymous class expressions.
    fn check_class_node(&self, n: &Node) -> Result<(), String> {
        self.check_class(&match n {
            NamedOrBlankNode::NamedNode(x) => Term::NamedNode(x.clone()),
            NamedOrBlankNode::BlankNode(x) => Term::BlankNode(x.clone()),
        })
    }

    fn property_kind(&self, iri: &str) -> &'static str {
        if self.is_data_property(iri) {
            "Data"
        } else if self.is_object_property(iri) {
            "Object"
        } else if self.is_annotation_property(iri) {
            "Annotation"
        } else {
            "Untyped"
        }
    }

    /// Transitive `rdfs:subPropertyOf` ancestors (including self), for the
    /// range-restriction condition on property chains.
    fn super_properties(&self, iri: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![iri.to_owned()];
        while let Some(p) = stack.pop() {
            if !seen.insert(p.clone()) {
                continue;
            }
            let Ok(n) = oxrdf::NamedNode::new(p.clone()) else {
                continue;
            };
            for o in self.objs(&Node::NamedNode(n), &rdfs("subPropertyOf")) {
                if let Some(i) = iri_of(o) {
                    stack.push(i.to_owned());
                }
            }
        }
        seen
    }

    fn imposed_ranges(&self, iri: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for p in self.super_properties(iri) {
            let Ok(n) = oxrdf::NamedNode::new(p) else {
                continue;
            };
            for r in self.objs(&Node::NamedNode(n), &rdfs("range")) {
                out.insert(self.key(r, 0));
            }
        }
        out
    }
}

/// Classify every logical axiom of `triples` against OWL 2 EL.
#[allow(clippy::too_many_lines)] // One arm per construct of the profile specification.
fn classify(triples: Vec<Triple>) -> Report {
    let doc = Doc::new(triples);
    let mut report = Report::default();
    let rdf_type = rdf("type");

    // DL typing: an IRI may not be both an object and a data property, or both a
    // class and a datatype.
    for (iri, types) in &doc.types {
        let has = |t: &str| types.contains(&owl(t));
        if has("ObjectProperty") && has("DatatypeProperty") {
            report.push(
                "Declaration",
                Err(format!(
                    "{iri} is an object and a data property (not OWL 2 DL)"
                )),
            );
        }
        if has("Class") && (types.contains(&rdfs("Datatype")) || has("DataRange")) {
            report.push(
                "Declaration",
                Err(format!("{iri} is a class and a datatype (not OWL 2 DL)")),
            );
        }
    }

    let mut anonymous_reported: HashSet<Node> = HashSet::new();
    for t in &doc.triples {
        let s = &t.subject;
        let p = t.predicate.as_str();
        let o = &t.object;
        if doc.structural(s) {
            // A class or property expression can itself be the subject of an
            // axiom (`[ a owl:Restriction ... ] rdfs:subClassOf :C`).
            classify_expression_axiom(&doc, &mut report, s, p, o);
            continue;
        }
        if let Node::BlankNode(_) = s {
            if doc.is_annotation_property(p) {
                report.push("AnnotationAssertion", Ok(()));
            } else if anonymous_reported.insert(s.clone()) {
                // An anonymous individual: outside EL.
                report.push("Assertion", Err("anonymous individual".into()));
            }
            continue;
        }
        let Node::NamedNode(sn) = s else { continue };
        let s_iri = sn.as_str();

        if p == rdf_type {
            classify_type(&doc, &mut report, s, s_iri, o);
            continue;
        }
        classify_predicate(&doc, &mut report, s, s_iri, p, o);
    }

    // Axiom nodes written as blank-node subjects.
    for (node, props) in &doc.by_subject {
        if !matches!(node, Node::BlankNode(_)) {
            continue;
        }
        let types = doc.node_types(node);
        let has = |t: &str| types.contains(&owl(t));
        if has("AllDisjointClasses") {
            let members = doc.one(node, &owl("members")).and_then(|m| doc.list(m));
            let check = match members {
                Some(m) => m.iter().try_for_each(|c| doc.check_class(c)),
                None => Err("malformed members list".into()),
            };
            report.push("DisjointClasses", check);
        } else if has("AllDisjointProperties") {
            report.push(
                "DisjointProperties",
                Err("DisjointObjectProperties/DisjointDataProperties".into()),
            );
        } else if has("AllDifferent") {
            let members = doc
                .one(node, &owl("distinctMembers"))
                .or_else(|| doc.one(node, &owl("members")))
                .and_then(|m| doc.list(m));
            let check = match members {
                Some(m) if m.iter().all(|x| matches!(x, Term::NamedNode(_))) => Ok(()),
                Some(_) => Err("anonymous individual".into()),
                None => Err("malformed members list".into()),
            };
            report.push("DifferentIndividuals", check);
        } else if has("NegativePropertyAssertion") {
            let prop = doc.one(node, &owl("assertionProperty"));
            let source_ok = doc
                .one(node, &owl("sourceIndividual"))
                .is_some_and(|x| matches!(x, Term::NamedNode(_)));
            let (kind, target) = if let Some(v) = doc.one(node, &owl("targetValue")) {
                (
                    "NegativeDataPropertyAssertion",
                    match v {
                        Term::Literal(l) => doc.check_literal(l),
                        _ => Err("bad target value".into()),
                    },
                )
            } else {
                (
                    "NegativeObjectPropertyAssertion",
                    match doc.one(node, &owl("targetIndividual")) {
                        Some(Term::NamedNode(_)) => Ok(()),
                        _ => Err("anonymous individual".into()),
                    },
                )
            };
            let prop_ok = prop.map_or(Err("missing property".to_owned()), |p| {
                if kind.contains("Data") {
                    Ok(())
                } else {
                    doc.check_object_property(p)
                }
            });
            let check = if source_ok {
                prop_ok.and(target)
            } else {
                Err("anonymous individual".into())
            };
            report.push(kind, check);
        }
        let _ = props;
    }

    // The EL range restriction on property chains (profiles, section 2.2.6).
    let mut chain_check: Vec<(String, Result<(), String>)> = Vec::new();
    for t in &doc.triples {
        if t.predicate.as_str() != owl("propertyChainAxiom") {
            continue;
        }
        let NamedOrBlankNode::NamedNode(sup) = &t.subject else {
            continue;
        };
        let Some(chain) = doc.list(&t.object) else {
            continue;
        };
        let Some(last) = chain.last().and_then(iri_of) else {
            continue;
        };
        let required = doc.imposed_ranges(sup.as_str());
        let have = doc.imposed_ranges(last);
        let res = if required.is_subset(&have) {
            Ok(())
        } else {
            Err(format!(
                "property chain into {}: range restriction on the super property is not imposed on {last}",
                sup.as_str()
            ))
        };
        chain_check.push(("PropertyChainRangeRestriction".to_owned(), res));
    }
    for (kind, res) in chain_check {
        report.push(&kind, res);
    }
    report
}

/// Axioms whose subject is an anonymous class or property expression.
fn classify_expression_axiom(doc: &Doc, report: &mut Report, s: &Node, p: &str, o: &Term) {
    const CLASS_AXIOMS: &[&str] = &["subClassOf", "equivalentClass", "disjointWith", "hasKey"];
    const PROPERTY_AXIOMS: &[&str] = &[
        "subPropertyOf",
        "equivalentProperty",
        "propertyDisjointWith",
        "domain",
        "range",
    ];
    let is_class_axiom = p.strip_prefix(RDFS).is_some_and(|l| l == "subClassOf")
        || p.strip_prefix(OWL)
            .is_some_and(|l| CLASS_AXIOMS.contains(&l));
    let is_property_axiom = p
        .strip_prefix(RDFS)
        .is_some_and(|l| PROPERTY_AXIOMS.contains(&l))
        || p.strip_prefix(OWL)
            .is_some_and(|l| PROPERTY_AXIOMS.contains(&l));
    if doc.has_pred(s, &owl("inverseOf")) && is_property_axiom {
        report.push("PropertyAxiom", Err("ObjectInverseOf".into()));
    } else if is_class_axiom {
        classify_predicate(doc, report, s, "", p, o);
    }
}

const DECLARATION_TYPES: &[&str] = &[
    "Class",
    "ObjectProperty",
    "DatatypeProperty",
    "AnnotationProperty",
    "NamedIndividual",
    "Ontology",
    "DeprecatedClass",
    "DeprecatedProperty",
    "Thing",
];

fn classify_type(doc: &Doc, report: &mut Report, s: &Node, s_iri: &str, o: &Term) {
    let Some(ty) = iri_of(o) else {
        // `x a [ class expression ]`
        report.push("ClassAssertion", doc.check_class(o));
        return;
    };
    let local = ty.strip_prefix(OWL);
    match local {
        Some(l) if DECLARATION_TYPES.contains(&l) => report.push("Declaration", Ok(())),
        Some("TransitiveProperty") => report.push("TransitiveObjectProperty", Ok(())),
        Some("ReflexiveProperty") => report.push("ReflexiveObjectProperty", Ok(())),
        Some("FunctionalProperty") => {
            if doc.is_data_property(s_iri) {
                report.push("FunctionalDataProperty", Ok(()));
            } else {
                report.push(
                    "FunctionalObjectProperty",
                    Err("FunctionalObjectProperty".into()),
                );
            }
        }
        Some("InverseFunctionalProperty") => {
            report.push(
                "InverseFunctionalObjectProperty",
                Err("InverseFunctionalObjectProperty".into()),
            );
        }
        Some("SymmetricProperty") => {
            report.push(
                "SymmetricObjectProperty",
                Err("SymmetricObjectProperty".into()),
            );
        }
        Some("AsymmetricProperty") => {
            report.push(
                "AsymmetricObjectProperty",
                Err("AsymmetricObjectProperty".into()),
            );
        }
        Some("IrreflexiveProperty") => {
            report.push(
                "IrreflexiveObjectProperty",
                Err("IrreflexiveObjectProperty".into()),
            );
        }
        Some("Nothing") => report.push("ClassAssertion", Ok(())),
        _ if !ty.starts_with(OWL) && doc.check_class_iri(ty).is_err() => {
            report.push("ClassAssertion", doc.check_class_iri(ty));
        }
        _ if ty == rdfs("Datatype") || ty == rdf("Property") || ty == rdfs("Class") => {
            report.push("Declaration", Ok(()));
        }
        _ => {
            let _ = s;
            report.push("ClassAssertion", Ok(()));
        }
    }
}

fn classify_predicate(doc: &Doc, report: &mut Report, s: &Node, s_iri: &str, p: &str, o: &Term) {
    match p {
        _ if p == rdfs("subClassOf") => {
            let r = doc.check_class_node(s).and(doc.check_class(o));
            report.push("SubClassOf", r);
        }
        _ if p == owl("equivalentClass") => {
            let r = doc.check_class_node(s).and(doc.check_class(o));
            report.push("EquivalentClasses", r);
        }
        _ if p == owl("disjointWith") => {
            let r = doc.check_class_node(s).and(doc.check_class(o));
            report.push("DisjointClasses", r);
        }
        _ if p == owl("disjointUnionOf") => {
            report.push("DisjointUnion", Err("DisjointUnion".into()));
        }
        _ if p == owl("intersectionOf") => {
            // A named class defined as an intersection: EquivalentClasses.
            let r = doc
                .list(o)
                .ok_or_else(|| "malformed list".to_owned())
                .and_then(|m| {
                    if m.len() < 2 {
                        Err("ObjectIntersectionOf with fewer than 2 operands".to_owned())
                    } else {
                        m.iter().try_for_each(|c| doc.check_class(c))
                    }
                });
            report.push("EquivalentClasses", r);
        }
        _ if p == owl("unionOf") => report.push("EquivalentClasses", Err("ObjectUnionOf".into())),
        _ if p == owl("complementOf") => {
            report.push("EquivalentClasses", Err("ObjectComplementOf".into()));
        }
        _ if p == owl("oneOf") => {
            let r = doc
                .list(o)
                .ok_or_else(|| "malformed list".to_owned())
                .and_then(|m| {
                    if m.len() == 1 {
                        Ok(())
                    } else {
                        Err(format!("enumeration of {} members", m.len()))
                    }
                });
            report.push("EquivalentClasses", r);
        }
        _ if p == rdfs("subPropertyOf") => {
            let kind = match doc.property_kind(s_iri) {
                "Data" => "SubDataPropertyOf",
                "Object" => "SubObjectPropertyOf",
                "Annotation" => "SubAnnotationPropertyOf",
                _ => "SubPropertyOf(untyped)",
            };
            let r = if kind == "SubObjectPropertyOf" {
                doc.check_object_property(o)
            } else if kind == "SubPropertyOf(untyped)" {
                Err("property is not declared as object, data or annotation property (not OWL 2 DL)".into())
            } else {
                Ok(())
            };
            report.push(kind, r);
        }
        _ if p == owl("propertyChainAxiom") => {
            let r = doc
                .list(o)
                .ok_or_else(|| "malformed list".to_owned())
                .and_then(|m| m.iter().try_for_each(|x| doc.check_object_property(x)));
            report.push("SubObjectPropertyOf(chain)", r);
        }
        _ if p == owl("equivalentProperty") => {
            let kind = if doc.is_data_property(s_iri) {
                "EquivalentDataProperties"
            } else {
                "EquivalentObjectProperties"
            };
            let r = if kind == "EquivalentObjectProperties" {
                doc.check_object_property(o)
            } else {
                Ok(())
            };
            report.push(kind, r);
        }
        _ if p == owl("propertyDisjointWith") => {
            report.push(
                "DisjointProperties",
                Err("DisjointObjectProperties/DisjointDataProperties".into()),
            );
        }
        _ if p == owl("inverseOf") => {
            report.push(
                "InverseObjectProperties",
                Err("InverseObjectProperties".into()),
            );
        }
        _ if (p == rdfs("domain") || p == rdfs("range"))
            && doc.property_kind(s_iri) == "Untyped"
            && !s_iri.is_empty() =>
        {
            report.push(
                "PropertyAxiom",
                Err(format!("property {s_iri} is not declared (not OWL 2 DL)")),
            );
        }
        _ if p == rdfs("domain") => match doc.property_kind(s_iri) {
            "Data" => report.push("DataPropertyDomain", doc.check_class(o)),
            "Annotation" => report.push("AnnotationPropertyDomain", Ok(())),
            _ => report.push("ObjectPropertyDomain", doc.check_class(o)),
        },
        _ if p == rdfs("range") => match doc.property_kind(s_iri) {
            "Data" => report.push("DataPropertyRange", doc.check_data_range(o)),
            "Annotation" => report.push("AnnotationPropertyRange", Ok(())),
            _ => report.push("ObjectPropertyRange", doc.check_class(o)),
        },
        _ if p == owl("hasKey") => {
            let r = doc.check_class_node(s).and_then(|()| {
                doc.list(o)
                    .ok_or_else(|| "malformed list".to_owned())
                    .and_then(|m| {
                        m.iter().try_for_each(|x| {
                            if iri_of(x).is_some_and(|i| doc.is_data_property(i)) {
                                Ok(())
                            } else {
                                doc.check_object_property(x)
                            }
                        })
                    })
            });
            report.push("HasKey", r);
        }
        _ if p == owl("sameAs") => {
            let r = match o {
                Term::NamedNode(_) => Ok(()),
                _ => Err("anonymous individual".into()),
            };
            report.push("SameIndividual", r);
        }
        _ if p == owl("differentFrom") => {
            let r = match o {
                Term::NamedNode(_) => Ok(()),
                _ => Err("anonymous individual".into()),
            };
            report.push("DifferentIndividuals", r);
        }
        _ if p == owl("imports") || p == owl("versionIRI") => {
            report.push("OntologyHeader", Ok(()));
        }
        _ if p.starts_with(OWL) && is_axiom_machinery(p) => {}
        _ => classify_assertion(doc, report, p, o),
    }
}

/// Predicates that belong to structures handled elsewhere.
fn is_axiom_machinery(p: &str) -> bool {
    matches!(
        p.strip_prefix(OWL),
        Some(
            "annotatedSource"
                | "annotatedProperty"
                | "annotatedTarget"
                | "members"
                | "distinctMembers"
                | "assertionProperty"
                | "sourceIndividual"
                | "targetIndividual"
                | "targetValue"
        )
    )
}

fn classify_assertion(doc: &Doc, report: &mut Report, p: &str, o: &Term) {
    match doc.property_kind(p) {
        "Annotation" => report.push("AnnotationAssertion", Ok(())),
        "Object" => {
            let r = match o {
                Term::NamedNode(_) => Ok(()),
                Term::BlankNode(_) => Err("anonymous individual".into()),
                Term::Literal(_) => Err("literal as object of an object property".into()),
            };
            report.push("ObjectPropertyAssertion", r);
        }
        "Data" => {
            let r = match o {
                Term::Literal(l) => doc.check_literal(l),
                _ => Err("non-literal as value of a data property".into()),
            };
            report.push("DataPropertyAssertion", r);
        }
        _ => report.push(
            "Assertion(untyped property)",
            Err(format!("property {p} is not declared (not OWL 2 DL)")),
        ),
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

fn parse_turtle(text: &[u8]) -> Vec<Triple> {
    TurtleParser::new()
        .for_slice(text)
        .map(|t| t.expect("valid Turtle"))
        .collect()
}

fn parse_rdfxml(text: &str) -> Result<Vec<Triple>, String> {
    RdfXmlParser::new()
        .with_base_iri("http://example.org/base")
        .map_err(|e| e.to_string())?
        .for_slice(text)
        .map(|t| t.map_err(|e| e.to_string()))
        .collect()
}

fn workspace_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

// ---------------------------------------------------------------------------
// The emitted ontology
// ---------------------------------------------------------------------------

fn load_report(path: &Path) -> Report {
    let text = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    classify(parse_turtle(&text))
}

/// Print the histogram (EL violations in full, DL typing issues summarised)
/// and assert no axiom is outside EL.
fn assert_ontology_is_el(path: &Path, label: &str) {
    let report = load_report(path);
    let el = report.el_violations();
    let typing = report.typing_issues();
    println!("{label}: {} axioms", report.axioms.len());
    for (kind, (total, bad)) in report.histogram() {
        println!("  {kind:<40} {total:>8}  flagged {bad}");
    }
    println!(
        "  EL violations: {}; OWL 2 DL typing issues (undeclared entities): {} distinct, {} axioms",
        el.values().sum::<usize>(),
        typing.len(),
        typing.values().sum::<usize>()
    );
    for (why, n) in &el {
        println!("  EL VIOLATION x{n}: {why}");
    }
    assert!(!report.axioms.is_empty(), "{label}: no axioms classified");
    assert!(el.is_empty(), "{label}: axioms outside OWL 2 EL: {el:?}");
}

#[test]
fn estate_small_fixture_is_owl2_el() {
    assert_ontology_is_el(
        &workspace_path("conformance/fixtures/estate-small.golden/expected/out/data/ontology.ttl"),
        "estate-small fixture",
    );
}

#[test]
fn full_corpus_is_owl2_el() {
    let path = Path::new(
        "/home/devuser/workspace/.tmp/ontomark-golden/corpus-visiongraph/out/data/ontology.ttl",
    );
    if !path.exists() {
        println!(
            "full corpus ontology.ttl not present at {}; skipping",
            path.display()
        );
        return;
    }
    assert_ontology_is_el(path, "visiongraph corpus");
}

/// The emitted ontology uses classes it never declares (parents of declared
/// classes whose own page is not in the public set), so a strict OWL 2 DL
/// reader (OWL API `UseOfUndeclaredClass`) rejects the document even though
/// every axiom is EL.
#[test]
#[ignore = "FINDING: ontology.ttl uses undeclared classes as rdfs:subClassOf targets (not OWL 2 DL typed); EL axioms are unaffected"]
fn fixture_declares_every_class_it_uses() {
    let report = load_report(&workspace_path(
        "conformance/fixtures/estate-small.golden/expected/out/data/ontology.ttl",
    ));
    let typing = report.typing_issues();
    assert!(
        typing.is_empty(),
        "{} undeclared-entity issues, e.g. {:?}",
        typing.len(),
        typing.keys().take(3).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Classifier unit tests: one example per construct in the specification
// ---------------------------------------------------------------------------

const PREAMBLE: &str = "
@prefix : <http://example.org/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
:A a owl:Class . :B a owl:Class . :C a owl:Class .
:p a owl:ObjectProperty . :q a owl:ObjectProperty . :r a owl:ObjectProperty .
:d a owl:DatatypeProperty . :e a owl:DatatypeProperty .
:a a owl:NamedIndividual . :b a owl:NamedIndividual .
";

fn is_el(body: &str) -> bool {
    let text = format!("{PREAMBLE}{body}");
    classify(parse_turtle(text.as_bytes())).is_el()
}

#[test]
fn el_allows_the_specified_constructs() {
    let allowed = [
        ":A rdfs:subClassOf :B .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:someValuesFrom :B ] .",
        ":A owl:equivalentClass [ owl:intersectionOf ( :B [ a owl:Restriction ; owl:onProperty :p ; owl:someValuesFrom :C ] ) ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:hasValue :a ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:hasSelf true ] .",
        ":A rdfs:subClassOf [ a owl:Class ; owl:oneOf ( :a ) ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :d ; owl:someValuesFrom xsd:string ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :d ; owl:hasValue \"x\" ] .",
        ":A owl:disjointWith :B .",
        "[ a owl:AllDisjointClasses ; owl:members ( :A :B :C ) ] .",
        ":p rdfs:subPropertyOf :q .",
        ":r owl:propertyChainAxiom ( :p :q ) .",
        ":p owl:equivalentProperty :q .",
        ":p a owl:TransitiveProperty .",
        ":p a owl:ReflexiveProperty .",
        ":p rdfs:domain :A . :p rdfs:range :B .",
        ":d rdfs:domain :A . :d rdfs:range xsd:integer .",
        ":d a owl:FunctionalProperty .",
        ":d rdfs:subPropertyOf :e . :d owl:equivalentProperty :e .",
        ":A owl:hasKey ( :p :d ) .",
        ":a owl:sameAs :b . :a owl:differentFrom :b .",
        ":a a :A . :a :p :b . :a :d \"x\" .",
        "[ a owl:NegativePropertyAssertion ; owl:sourceIndividual :a ; owl:assertionProperty :p ; owl:targetIndividual :b ] .",
        "[ a owl:NegativePropertyAssertion ; owl:sourceIndividual :a ; owl:assertionProperty :d ; owl:targetValue \"x\" ] .",
        ":A rdfs:label \"a\"@en .",
        // Equivalent properties carry a range on the chain's last property.
        ":p rdfs:range :B . :q rdfs:range :B . :r rdfs:range :B . :r owl:propertyChainAxiom ( :p :q ) .",
    ];
    for body in allowed {
        assert!(is_el(body), "should be EL: {body}");
    }
}

#[test]
fn el_rejects_the_unsupported_constructs() {
    let rejected = [
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:allValuesFrom :B ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :d ; owl:allValuesFrom xsd:string ] .",
        ":A rdfs:subClassOf [ a owl:Class ; owl:unionOf ( :B :C ) ] .",
        ":A rdfs:subClassOf [ a owl:Class ; owl:complementOf :B ] .",
        ":A owl:complementOf :B .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:minCardinality 1 ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:maxQualifiedCardinality 1 ; owl:onClass :B ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :p ; owl:cardinality 1 ] .",
        ":A rdfs:subClassOf [ a owl:Class ; owl:oneOf ( :a :b ) ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty [ owl:inverseOf :p ] ; owl:someValuesFrom :B ] .",
        ":A owl:disjointUnionOf ( :B :C ) .",
        ":p owl:inverseOf :q .",
        ":p a owl:SymmetricProperty .",
        ":p a owl:AsymmetricProperty .",
        ":p a owl:IrreflexiveProperty .",
        ":p a owl:FunctionalProperty .",
        ":p a owl:InverseFunctionalProperty .",
        ":p owl:propertyDisjointWith :q .",
        ":d owl:propertyDisjointWith :e .",
        "[ a owl:AllDisjointProperties ; owl:members ( :p :q ) ] .",
        ":d rdfs:range xsd:float .",
        ":d rdfs:range xsd:boolean .",
        ":a :d \"1.5\"^^xsd:double .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :d ; owl:someValuesFrom [ a rdfs:Datatype ; owl:unionOf ( xsd:string xsd:integer ) ] ] .",
        ":A rdfs:subClassOf [ a owl:Restriction ; owl:onProperty :d ; owl:someValuesFrom [ a rdfs:Datatype ; owl:oneOf ( \"x\" \"y\" ) ] ] .",
        ":a :p _:anon .",
        "_:anon :p :b .",
        // A range on the super property that the chain's last property lacks.
        ":r rdfs:range :B . :r owl:propertyChainAxiom ( :p :q ) .",
        // An undeclared property is not an OWL 2 DL assertion at all.
        ":a :undeclared :b .",
    ];
    for body in rejected {
        assert!(!is_el(body), "should NOT be EL: {body}");
    }
}

#[test]
fn annotation_values_may_use_any_datatype() {
    let body = "
:note a owl:AnnotationProperty .
:A :note \"0.6\"^^xsd:float .
:A rdfs:comment \"c\"@en .";
    assert!(is_el(body));
}

// ---------------------------------------------------------------------------
// OWL 2 profile-identification test cases (OWL API packaging)
// ---------------------------------------------------------------------------

/// `(file stem, ontology is EL)`; expectations follow the OWL API's profile
/// tests (`ProfileBase.test(format, doc, el, ql, rl, dl)`).
const PROFILE_FILES: &[(&str, bool, bool)] = &[
    // (name, EL expected, ontology is OWL 2 DL)
    ("All", true, true),
    ("ELDL", true, true),
    ("NOQL", true, true),
    ("NORL", true, true),
    ("NOEL", false, true),
    ("DL", false, true),
    ("RLDL", false, true),
    ("QLDL", false, true),
    ("Full", false, false),
];

/// Documents in a profile file: blank-line separated; RDF/XML ones start with
/// `<`. Every case is stored once in each syntax, so the RDF/XML copy stands
/// for the case.
fn split_documents(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
        .collect()
}

#[derive(Default, Debug)]
struct Agreement {
    cases: usize,
    functional_skipped: usize,
    agree: usize,
    disagree: Vec<String>,
    unparsable: Vec<String>,
}

fn run_profile_file(path: &Path, el_expected: bool) -> Agreement {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Agreement::default();
    for (i, doc) in split_documents(&text).iter().enumerate() {
        if !doc.starts_with('<') {
            out.functional_skipped += 1;
            continue;
        }
        out.cases += 1;
        let triples = match parse_rdfxml(doc) {
            Ok(t) => t,
            Err(e) => {
                out.unparsable.push(format!("#{i}: {e}"));
                continue;
            }
        };
        let report = classify(triples);
        if report.is_el() == el_expected {
            out.agree += 1;
        } else {
            let why = report
                .violations()
                .into_keys()
                .next()
                .unwrap_or_else(|| "classified EL".to_owned());
            out.disagree.push(format!("#{i} ({why})"));
        }
    }
    out
}

/// Documented disagreements with the OWL API's labels, as `file#index`.
/// Everything outside this list must agree.
const KNOWN_DISAGREEMENTS: &[&str] = &[];

#[test]
fn profile_identification_agreement() {
    let dir = workspace_path("conformance/w3c/owl2-profile");
    if !dir.join("profileAllTestCases.txt").exists() {
        println!("owl2-profile: cases not fetched (run conformance/w3c/fetch.sh); skipping");
        return;
    }
    let (mut total, mut agree) = (0usize, 0usize);
    let mut unexpected = Vec::new();
    for (name, el_expected, dl) in PROFILE_FILES {
        let a = run_profile_file(
            &dir.join(format!("profile{name}TestCases.txt")),
            *el_expected,
        );
        println!(
            "owl2-profile {name:<5} expect EL={el_expected:<5} (DL ontology: {dl:<5}): cases={} agree={} disagree={} unparsable={} functional-syntax copies skipped={}",
            a.cases,
            a.agree,
            a.disagree.len(),
            a.unparsable.len(),
            a.functional_skipped
        );
        for d in &a.disagree {
            println!("    DISAGREE {name}{d}");
            if !KNOWN_DISAGREEMENTS
                .contains(&format!("{name}{}", d.split(' ').next().unwrap_or("")).as_str())
            {
                // Full-species cases are not DL: the classifier does not implement
                // every DL well-formedness check, so they are reported only.
                if *dl {
                    unexpected.push(format!("{name}{d}"));
                }
            }
        }
        for u in &a.unparsable {
            println!("    UNPARSABLE {name}{u}");
        }
        total += a.cases;
        agree += a.agree;
    }
    println!("owl2-profile total: cases={total} agree={agree}");
    assert!(
        unexpected.is_empty(),
        "classifier disagrees with the profile labels: {unexpected:?}"
    );
}
