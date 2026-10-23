//! The derivation certificate of a built graph (ADR-OM-009).
//!
//! [`certify`] spells the compiler's [`Graph`] into N-Triples terms and hands
//! it to the `rdfs-certificate` crate, which records every RDFS step in the
//! `oo-cert/1` format an independent Lean 4 checker re-verifies. The
//! certificate is the **warrant** for the closure `ontology-inferred.ttl`
//! carries: [`parity`] checks that the certified named closure is exactly the
//! classifier's, which is structural on this compiler's output (restrictions
//! are emitted only on the superclass side of `rdfs:subClassOf`, so every
//! named entailment is subsumption transitivity) and is enforced by the build
//! so that a generation is never published with a closure its certificate
//! does not cover.

use std::collections::BTreeSet;

use rdfs_certificate::nt;
pub use rdfs_certificate::{
    consequence_delta, Certificate, ConsequenceDelta, Summary, CONSERVATIVE, FORMAT,
    NOT_CONSERVATIVE, RULE_TABLE,
};

use crate::build::turtle::{Graph, Subject, Term};
use crate::reason::Reasoning;

/// Spell every triple of `graph` as N-Triples terms, in the graph's order.
#[must_use]
pub fn spell(graph: &Graph) -> Vec<(String, String, String)> {
    graph
        .iter()
        .map(|(s, p, o)| {
            let s = match s {
                Subject::Iri(i) => nt::iri(i),
                Subject::Blank(b) => nt::blank(b),
            };
            let o = match o {
                Term::Iri(i) => nt::iri(i),
                Term::Blank(b) => nt::blank(b),
                Term::Literal {
                    value,
                    lang,
                    datatype,
                } => nt::literal(value, lang.as_deref(), datatype.as_deref()),
            };
            (s, nt::iri(p), o)
        })
        .collect()
}

/// Certify `graph` under the RDFS rule table.
#[must_use]
pub fn certify(graph: &Graph) -> Certificate {
    let spelled = spell(graph);
    rdfs_certificate::certify(
        spelled
            .iter()
            .map(|(s, p, o)| (s.as_str(), p.as_str(), o.as_str())),
    )
}

/// The certificate's named `rdfs:subClassOf` closure against the classifier's
/// inferred subsumptions: `(certified, classifier, identical)`.
#[must_use]
pub fn parity(cert: &Certificate, reasoning: &Reasoning) -> (usize, usize, bool) {
    let classifier: BTreeSet<(String, String)> = reasoning.inferred.iter().cloned().collect();
    let certified = cert.named_subclass_closure();
    let same = certified == classifier;
    (certified.len(), classifier.len(), same)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SC: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
    #[cfg(feature = "whelk-el")]
    const TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

    fn ex(l: &str) -> String {
        format!("urn:ex:{l}")
    }

    #[test]
    fn spelling_escapes_literals_and_keeps_blank_nodes() {
        let mut g = Graph::new();
        let b = g.fresh_blank();
        g.add_iri(&ex("a"), SC, Term::Blank(b.clone()));
        g.add(
            Subject::Blank(b),
            "http://www.w3.org/2000/01/rdf-schema#label",
            Term::lang("tab\there", "en"),
        );
        let s = spell(&g);
        assert_eq!(s.len(), 2);
        assert!(s[0].2.starts_with("_:"));
        assert_eq!(s[1].2, "\"tab\\there\"@en");
        assert_eq!(certify(&g).derivation_count(), 0);
    }

    #[cfg(feature = "whelk-el")]
    #[test]
    fn whelk_and_the_certificate_agree_on_the_named_closure() {
        let owl = |l: &str| Term::Iri(format!("http://www.w3.org/2002/07/owl#{l}"));
        let mut g = Graph::new();
        for c in ["a", "b", "c", "d", "e", "x"] {
            g.add_iri(&ex(c), TYPE, owl("Class"));
        }
        g.add_iri(&ex("p"), TYPE, owl("ObjectProperty"));
        for (s, o) in [("a", "b"), ("b", "c"), ("c", "d"), ("x", "c"), ("e", "a")] {
            g.add_iri(&ex(s), SC, Term::Iri(ex(o)));
        }
        let r = g.fresh_blank();
        g.add_iri(&ex("b"), SC, Term::Blank(r.clone()));
        g.add(
            Subject::Blank(r.clone()),
            "http://www.w3.org/2002/07/owl#onProperty",
            Term::Iri(ex("p")),
        );
        g.add(
            Subject::Blank(r),
            "http://www.w3.org/2002/07/owl#someValuesFrom",
            Term::Iri(ex("d")),
        );
        let reasoning = crate::reason::reason(&g, &crate::whelk::WhelkEl).unwrap();
        assert!(!reasoning.inferred.is_empty());
        let (certified, whelk, same) = parity(&certify(&g), &reasoning);
        assert!(same, "certified {certified} vs whelk {whelk}");
    }
}
