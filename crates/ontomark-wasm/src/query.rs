//! Local, read-only SPARQL over the compiled graph (EXP-12).
//!
//! Parsing and evaluation are Oxigraph's `spargebra` and `spareval` over an
//! in-memory [`oxrdf::Dataset`]; this module only decides what may run and
//! shapes the results. The dataset is rebuilt from Markdown on demand and is
//! never persisted, so it cannot drift from the source.
//!
//! Accepted: `SELECT`, `ASK` and `CONSTRUCT` with basic graph patterns,
//! property paths, joins, `OPTIONAL`, `UNION`, `MINUS`, `FILTER` (including
//! `EXISTS`/`NOT EXISTS`), `BIND`, `VALUES`, `GRAPH`, sub-selects, aggregates
//! with `GROUP BY`, `ORDER BY`, `DISTINCT`/`REDUCED`, `LIMIT`/`OFFSET`, and
//! the built-in function library.
//!
//! Refused with a diagnostic: SPARQL Update, `SERVICE` anywhere in the
//! algebra, `FROM`/`FROM NAMED`, `DESCRIBE`, custom (extension) functions and
//! any algebra form not listed above. Results are capped at a row limit.
//!
//! The query's default graph is the selected [`DatasetSelection`]; the
//! asserted and inferred graphs are also available by name as
//! [`ASSERTED_GRAPH`] and [`INFERRED_GRAPH`].

use std::collections::BTreeMap;

use oxrdf::Term;
#[cfg(test)]
use oxrdf::{Dataset, GraphName, NamedNode, Quad, Triple};
use serde::{Deserialize, Serialize};
use spareval::{QueryEvaluator, QueryResults};
use spargebra::algebra::{
    AggregateExpression, Expression, Function, GraphPattern, OrderExpression,
};
use spargebra::{Query, SparqlParser};

/// Named graph holding the asserted triples.
pub const ASSERTED_GRAPH: &str = "urn:ontomark:graph:asserted";
/// Named graph holding the inferred triples.
pub const INFERRED_GRAPH: &str = "urn:ontomark:graph:inferred";
/// Row cap applied when the request gives none.
pub const DEFAULT_ROW_LIMIT: usize = 1000;
/// Largest row cap a request may ask for.
pub const MAX_ROW_LIMIT: usize = 100_000;

/// Which triples form the query's default graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetSelection {
    /// Triples compiled from frontmatter.
    #[default]
    Asserted,
    /// Triples entailed by the last classification of this generation.
    Inferred,
    /// Both.
    Combined,
}

/// An RDF term on the wire.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WireTerm {
    /// An IRI.
    Iri {
        /// The IRI.
        value: String,
    },
    /// A blank node.
    Bnode {
        /// Its label.
        value: String,
    },
    /// A literal.
    Literal {
        /// Lexical form.
        value: String,
        /// Datatype IRI.
        datatype: String,
        /// Language tag.
        #[serde(skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
    },
}

impl From<&Term> for WireTerm {
    fn from(t: &Term) -> Self {
        match t {
            Term::NamedNode(n) => Self::Iri {
                value: n.as_str().to_owned(),
            },
            Term::BlankNode(b) => Self::Bnode {
                value: b.as_str().to_owned(),
            },
            Term::Literal(l) => Self::Literal {
                value: l.value().to_owned(),
                datatype: l.datatype().as_str().to_owned(),
                lang: l.language().map(str::to_owned),
            },
            #[allow(unreachable_patterns)]
            _ => Self::Literal {
                value: t.to_string(),
                datatype: "urn:ontomark:unsupported-term".into(),
                lang: None,
            },
        }
    }
}

/// The result of a query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "form", rename_all = "lowercase")]
pub enum QueryOutput {
    /// `SELECT` solutions.
    Select {
        /// Projected variables, in order.
        variables: Vec<String>,
        /// Rows; an unbound variable is absent from its row.
        rows: Vec<BTreeMap<String, WireTerm>>,
        /// `true` when the row cap cut the result.
        truncated: bool,
    },
    /// `ASK` answer.
    Ask {
        /// The answer.
        boolean: bool,
    },
    /// `CONSTRUCT` triples, `[subject, predicate, object]`.
    Construct {
        /// Triples.
        triples: Vec<[WireTerm; 3]>,
        /// `true` when the row cap cut the result.
        truncated: bool,
    },
}

/// Why a query was refused or failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueryError {
    /// Not valid SPARQL 1.1 query syntax.
    #[error("syntax: {0}")]
    Syntax(String),
    /// SPARQL Update was submitted.
    #[error("SPARQL Update is not available: queries never change the vault")]
    UpdateForbidden,
    /// A `SERVICE` clause appeared.
    #[error("SERVICE is disabled: queries are local only")]
    ServiceForbidden,
    /// `FROM` / `FROM NAMED` appeared.
    #[error("FROM and FROM NAMED are not supported: choose the dataset with the asserted/inferred/combined selector")]
    DatasetClause,
    /// A recognised but unsupported form.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Evaluation failed.
    #[error("evaluation: {0}")]
    Evaluation(String),
}

impl QueryError {
    /// Stable machine code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Syntax(_) => "query_syntax",
            Self::UpdateForbidden => "update_forbidden",
            Self::ServiceForbidden => "service_forbidden",
            Self::DatasetClause => "dataset_clause",
            Self::Unsupported(_) => "query_unsupported",
            Self::Evaluation(_) => "query_evaluation",
        }
    }
}

/// Parse `text` and check it against the supported subset.
///
/// # Errors
/// Any [`QueryError`] except `Evaluation`.
pub fn parse_checked(text: &str) -> Result<Query, QueryError> {
    let query = match SparqlParser::new().parse_query(text) {
        Ok(q) => q,
        Err(e) => {
            return Err(if SparqlParser::new().parse_update(text).is_ok() {
                QueryError::UpdateForbidden
            } else {
                QueryError::Syntax(e.to_string())
            })
        }
    };
    let (dataset, pattern) = match &query {
        Query::Select {
            dataset, pattern, ..
        }
        | Query::Ask {
            dataset, pattern, ..
        } => (dataset, pattern),
        Query::Construct {
            dataset, pattern, ..
        } => (dataset, pattern),
        Query::Describe { .. } => return Err(QueryError::Unsupported("DESCRIBE".into())),
    };
    if dataset.is_some() {
        return Err(QueryError::DatasetClause);
    }
    check_pattern(pattern)?;
    Ok(query)
}

fn check_pattern(p: &GraphPattern) -> Result<(), QueryError> {
    match p {
        GraphPattern::Bgp { .. } | GraphPattern::Path { .. } | GraphPattern::Values { .. } => {
            Ok(())
        }
        GraphPattern::Join { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            check_pattern(left)?;
            check_pattern(right)
        }
        GraphPattern::LeftJoin {
            left,
            right,
            expression,
        } => {
            check_pattern(left)?;
            check_pattern(right)?;
            expression.as_ref().map_or(Ok(()), check_expr)
        }
        GraphPattern::Filter { expr, inner } => {
            check_expr(expr)?;
            check_pattern(inner)
        }
        GraphPattern::Extend {
            inner, expression, ..
        } => {
            check_expr(expression)?;
            check_pattern(inner)
        }
        GraphPattern::OrderBy { inner, expression } => {
            for o in expression {
                match o {
                    OrderExpression::Asc(e) | OrderExpression::Desc(e) => check_expr(e)?,
                }
            }
            check_pattern(inner)
        }
        GraphPattern::Group {
            inner, aggregates, ..
        } => {
            for (_, a) in aggregates {
                if let AggregateExpression::FunctionCall { expr, .. } = a {
                    check_expr(expr)?;
                }
            }
            check_pattern(inner)
        }
        GraphPattern::Graph { inner, .. }
        | GraphPattern::Project { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::Slice { inner, .. } => check_pattern(inner),
        GraphPattern::Service { .. } => Err(QueryError::ServiceForbidden),
        #[allow(unreachable_patterns)]
        other => Err(QueryError::Unsupported(format!("graph pattern {other}"))),
    }
}

fn check_expr(e: &Expression) -> Result<(), QueryError> {
    match e {
        Expression::NamedNode(_)
        | Expression::Literal(_)
        | Expression::Variable(_)
        | Expression::Bound(_) => Ok(()),
        Expression::Or(a, b)
        | Expression::And(a, b)
        | Expression::Equal(a, b)
        | Expression::SameTerm(a, b)
        | Expression::Greater(a, b)
        | Expression::GreaterOrEqual(a, b)
        | Expression::Less(a, b)
        | Expression::LessOrEqual(a, b)
        | Expression::Add(a, b)
        | Expression::Subtract(a, b)
        | Expression::Multiply(a, b)
        | Expression::Divide(a, b) => {
            check_expr(a)?;
            check_expr(b)
        }
        Expression::UnaryPlus(a) | Expression::UnaryMinus(a) | Expression::Not(a) => check_expr(a),
        Expression::In(a, list) => {
            check_expr(a)?;
            list.iter().try_for_each(check_expr)
        }
        Expression::If(a, b, c) => {
            check_expr(a)?;
            check_expr(b)?;
            check_expr(c)
        }
        Expression::Coalesce(list) => list.iter().try_for_each(check_expr),
        Expression::FunctionCall(f, args) => {
            if let Function::Custom(iri) = f {
                return Err(QueryError::Unsupported(format!("extension function {iri}")));
            }
            args.iter().try_for_each(check_expr)
        }
        Expression::Exists(p) => check_pattern(p),
        #[allow(unreachable_patterns)]
        other => Err(QueryError::Unsupported(format!("expression {other}"))),
    }
}

/// `true` when the query names a graph (`GRAPH ?g { … }` anywhere, including
/// inside `EXISTS`), so the named graphs must be materialised.
#[must_use]
pub fn uses_graph(query: &Query) -> bool {
    let pattern = match query {
        Query::Select { pattern, .. }
        | Query::Ask { pattern, .. }
        | Query::Construct { pattern, .. } => pattern,
        Query::Describe { pattern, .. } => pattern,
    };
    let mut found = false;
    pattern_uses_graph(pattern, &mut found);
    found
}

fn pattern_uses_graph(p: &GraphPattern, found: &mut bool) {
    if *found {
        return;
    }
    match p {
        GraphPattern::Graph { .. } => *found = true,
        GraphPattern::Bgp { .. } | GraphPattern::Path { .. } | GraphPattern::Values { .. } => {}
        GraphPattern::Join { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            pattern_uses_graph(left, found);
            pattern_uses_graph(right, found);
        }
        GraphPattern::LeftJoin {
            left,
            right,
            expression,
        } => {
            pattern_uses_graph(left, found);
            pattern_uses_graph(right, found);
            if let Some(e) = expression {
                expr_uses_graph(e, found);
            }
        }
        GraphPattern::Filter { expr, inner } => {
            expr_uses_graph(expr, found);
            pattern_uses_graph(inner, found);
        }
        GraphPattern::Extend {
            inner, expression, ..
        } => {
            expr_uses_graph(expression, found);
            pattern_uses_graph(inner, found);
        }
        GraphPattern::OrderBy { inner, .. }
        | GraphPattern::Group { inner, .. }
        | GraphPattern::Project { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::Slice { inner, .. }
        | GraphPattern::Service { inner, .. } => pattern_uses_graph(inner, found),
        // Any other (feature-gated) form: assume it may name graphs.
        #[allow(unreachable_patterns)]
        _ => *found = true,
    }
}

fn expr_uses_graph(e: &Expression, found: &mut bool) {
    if *found {
        return;
    }
    match e {
        Expression::Exists(p) => pattern_uses_graph(p, found),
        Expression::Or(a, b)
        | Expression::And(a, b)
        | Expression::Equal(a, b)
        | Expression::SameTerm(a, b)
        | Expression::Greater(a, b)
        | Expression::GreaterOrEqual(a, b)
        | Expression::Less(a, b)
        | Expression::LessOrEqual(a, b)
        | Expression::Add(a, b)
        | Expression::Subtract(a, b)
        | Expression::Multiply(a, b)
        | Expression::Divide(a, b) => {
            expr_uses_graph(a, found);
            expr_uses_graph(b, found);
        }
        Expression::UnaryPlus(a) | Expression::UnaryMinus(a) | Expression::Not(a) => {
            expr_uses_graph(a, found)
        }
        Expression::If(a, b, c) => {
            expr_uses_graph(a, found);
            expr_uses_graph(b, found);
            expr_uses_graph(c, found);
        }
        Expression::In(a, list) => {
            expr_uses_graph(a, found);
            list.iter().for_each(|x| expr_uses_graph(x, found));
        }
        Expression::Coalesce(list) | Expression::FunctionCall(_, list) => {
            list.iter().for_each(|x| expr_uses_graph(x, found));
        }
        _ => {}
    }
}

/// Reference dataset built with [`oxrdf::Dataset`], used only by the
/// differential tests that check [`crate::store::Store`] against it.
#[cfg(test)]
#[must_use]
pub fn dataset(
    asserted: &[Triple],
    inferred: &[Triple],
    selection: DatasetSelection,
    named_graphs: bool,
) -> Dataset {
    let mut ds = Dataset::new();
    let named = |iri: &str| GraphName::NamedNode(NamedNode::new_unchecked(iri));
    for t in asserted.iter().filter(|_| named_graphs) {
        ds.insert(&Quad::new(
            t.subject.clone(),
            t.predicate.clone(),
            t.object.clone(),
            named(ASSERTED_GRAPH),
        ));
    }
    if selection != DatasetSelection::Inferred {
        for t in asserted {
            ds.insert(&Quad::new(
                t.subject.clone(),
                t.predicate.clone(),
                t.object.clone(),
                GraphName::DefaultGraph,
            ));
        }
    }
    for t in inferred.iter().filter(|_| named_graphs) {
        ds.insert(&Quad::new(
            t.subject.clone(),
            t.predicate.clone(),
            t.object.clone(),
            named(INFERRED_GRAPH),
        ));
    }
    if selection != DatasetSelection::Asserted {
        for t in inferred {
            ds.insert(&Quad::new(
                t.subject.clone(),
                t.predicate.clone(),
                t.object.clone(),
                GraphName::DefaultGraph,
            ));
        }
    }
    ds
}

/// Evaluate a checked query over `ds`, keeping at most `limit` rows.
///
/// # Errors
/// [`QueryError::Evaluation`] when `spareval` fails.
pub fn evaluate<'a>(
    query: &Query,
    ds: impl spareval::QueryableDataset<'a>,
    limit: usize,
) -> Result<QueryOutput, QueryError> {
    let limit = limit.clamp(1, MAX_ROW_LIMIT);
    let eval_err = |e: spareval::QueryEvaluationError| QueryError::Evaluation(e.to_string());
    let results = QueryEvaluator::new()
        .prepare(query)
        .execute(ds)
        .map_err(eval_err)?;
    match results {
        QueryResults::Boolean(boolean) => Ok(QueryOutput::Ask { boolean }),
        QueryResults::Solutions(solutions) => {
            let variables: Vec<String> = solutions
                .variables()
                .iter()
                .map(|v| v.as_str().to_owned())
                .collect();
            let mut rows = Vec::new();
            let mut truncated = false;
            for solution in solutions {
                let solution = solution.map_err(eval_err)?;
                if rows.len() == limit {
                    truncated = true;
                    break;
                }
                rows.push(
                    solution
                        .iter()
                        .map(|(v, t)| (v.as_str().to_owned(), WireTerm::from(t)))
                        .collect(),
                );
            }
            Ok(QueryOutput::Select {
                variables,
                rows,
                truncated,
            })
        }
        QueryResults::Graph(triples) => {
            let mut out = Vec::new();
            let mut truncated = false;
            for t in triples {
                let t = t.map_err(eval_err)?;
                if out.len() == limit {
                    truncated = true;
                    break;
                }
                let s: Term = t.subject.into();
                out.push([
                    WireTerm::from(&s),
                    WireTerm::from(&Term::NamedNode(t.predicate)),
                    WireTerm::from(&t.object),
                ]);
            }
            Ok(QueryOutput::Construct {
                triples: out,
                truncated,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal, NamedOrBlankNode};

    fn t(s: &str, p: &str, o: Term) -> Triple {
        Triple::new(
            NamedOrBlankNode::NamedNode(NamedNode::new_unchecked(s)),
            NamedNode::new_unchecked(p),
            o,
        )
    }

    fn iri(s: &str) -> Term {
        NamedNode::new_unchecked(s).into()
    }

    #[test]
    fn refusals_have_specific_codes() {
        let cases = [
            ("INSERT DATA { <urn:a> <urn:b> <urn:c> }", "update_forbidden"),
            ("SELECT * WHERE { SERVICE <http://example.org/sparql> { ?s ?p ?o } }", "service_forbidden"),
            ("SELECT * WHERE { ?s ?p ?o OPTIONAL { SERVICE <http://x.example/> { ?s ?q ?r } } }", "service_forbidden"),
            ("SELECT * WHERE { ?s ?p ?o FILTER EXISTS { SERVICE <http://x.example/> { ?s ?q ?r } } }", "service_forbidden"),
            ("SELECT * WHERE { { SELECT ?s WHERE { SERVICE SILENT <http://x.example/> { ?s ?q ?r } } } }", "service_forbidden"),
            ("SELECT * FROM <urn:g> WHERE { ?s ?p ?o }", "dataset_clause"),
            ("DESCRIBE <urn:a>", "query_unsupported"),
            ("SELECT * WHERE { ?s ?p ?o FILTER(<urn:fn>(?o)) }", "query_unsupported"),
            ("SELEKT nonsense", "query_syntax"),
        ];
        for (q, code) in cases {
            let err = parse_checked(q).expect_err(q);
            assert_eq!(err.code(), code, "{q}: {err}");
        }
    }

    #[test]
    fn select_with_filter_order_limit_over_selected_graph() {
        let asserted = vec![
            t(
                "urn:a",
                "urn:q",
                Literal::new_typed_literal("0.9", oxrdf::vocab::xsd::DECIMAL).into(),
            ),
            t(
                "urn:b",
                "urn:q",
                Literal::new_typed_literal("0.2", oxrdf::vocab::xsd::DECIMAL).into(),
            ),
            t(
                "urn:c",
                "urn:q",
                Literal::new_typed_literal("0.5", oxrdf::vocab::xsd::DECIMAL).into(),
            ),
        ];
        let inferred = vec![t("urn:a", "urn:sub", iri("urn:top"))];
        let q = parse_checked(
            "SELECT ?s ?v WHERE { ?s <urn:q> ?v FILTER(?v > 0.3) } ORDER BY DESC(?v) LIMIT 5",
        )
        .unwrap();
        let out = evaluate(
            &q,
            &dataset(&asserted, &inferred, DatasetSelection::Asserted, true),
            10,
        )
        .unwrap();
        let QueryOutput::Select {
            rows, truncated, ..
        } = out
        else {
            panic!()
        };
        let subjects: Vec<_> = rows.iter().map(|r| r["s"].clone()).collect();
        assert_eq!(
            subjects,
            vec![
                WireTerm::Iri {
                    value: "urn:a".into()
                },
                WireTerm::Iri {
                    value: "urn:c".into()
                }
            ]
        );
        assert!(!truncated);
        // Inferred-only selection does not see asserted triples, but the named graph does.
        let ask = parse_checked("ASK { <urn:a> <urn:q> ?v }").unwrap();
        let inf = dataset(&asserted, &inferred, DatasetSelection::Inferred, true);
        assert_eq!(
            evaluate(&ask, &inf, 1).unwrap(),
            QueryOutput::Ask { boolean: false }
        );
        let named = parse_checked(&format!(
            "ASK {{ GRAPH <{ASSERTED_GRAPH}> {{ <urn:a> <urn:q> ?v }} }}"
        ))
        .unwrap();
        assert_eq!(
            evaluate(&named, &inf, 1).unwrap(),
            QueryOutput::Ask { boolean: true }
        );
    }

    #[test]
    fn graph_detection_finds_nested_graph_patterns() {
        let yes = [
            "SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }",
            "ASK { ?s ?p ?o FILTER EXISTS { GRAPH <urn:ontomark:graph:inferred> { ?s ?p ?o } } }",
            "SELECT * WHERE { ?s ?p ?o OPTIONAL { GRAPH ?g { ?s ?q ?r } } }",
        ];
        for q in yes {
            assert!(uses_graph(&parse_checked(q).unwrap()), "{q}");
        }
        assert!(!uses_graph(
            &parse_checked("SELECT * WHERE { ?s ?p ?o }").unwrap()
        ));
    }

    #[test]
    fn row_cap_truncates() {
        let asserted: Vec<_> = (0..20)
            .map(|i| t(&format!("urn:s{i}"), "urn:p", iri("urn:o")))
            .collect();
        let q = parse_checked("SELECT ?s WHERE { ?s ?p ?o }").unwrap();
        let QueryOutput::Select {
            rows, truncated, ..
        } = evaluate(
            &q,
            &dataset(&asserted, &[], DatasetSelection::Asserted, false),
            7,
        )
        .unwrap()
        else {
            panic!()
        };
        assert_eq!(rows.len(), 7);
        assert!(truncated);
    }

    /// Differential test: the compact store answers exactly like oxrdf::Dataset.
    #[test]
    fn store_matches_oxrdf_dataset_on_every_pattern_shape() {
        use crate::store::Store;
        let lit =
            |v: &str| -> Term { Literal::new_typed_literal(v, oxrdf::vocab::xsd::DECIMAL).into() };
        let mut asserted = Vec::new();
        for i in 0..30 {
            asserted.push(t(
                &format!("urn:s{i}"),
                "urn:p",
                iri(&format!("urn:s{}", (i + 1) % 30)),
            ));
            asserted.push(t(&format!("urn:s{i}"), "urn:q", lit(&format!("0.{i}"))));
            if i % 3 == 0 {
                asserted.push(t(&format!("urn:s{i}"), "urn:r", iri("urn:s0")));
            }
        }
        let inferred: Vec<_> = (0..10)
            .map(|i| t(&format!("urn:s{i}"), "urn:sub", iri("urn:top")))
            .collect();
        let queries = [
            "SELECT * WHERE { ?s ?p ?o }",
            "SELECT * WHERE { <urn:s3> ?p ?o }",
            "SELECT * WHERE { <urn:s3> <urn:p> ?o }",
            "SELECT * WHERE { <urn:s3> ?p <urn:s0> }",
            "SELECT * WHERE { ?s <urn:p> ?o }",
            "SELECT * WHERE { ?s <urn:r> <urn:s0> }",
            "SELECT * WHERE { ?s ?p <urn:s0> }",
            "ASK { <urn:s3> <urn:r> <urn:s0> }",
            "SELECT ?s WHERE { ?s <urn:q> ?v FILTER(?v > 0.15) } ORDER BY ?s",
            "SELECT ?a ?c WHERE { ?a <urn:p> ?b . ?b <urn:p> ?c . OPTIONAL { ?a <urn:r> ?x } }",
            "SELECT (COUNT(*) AS ?n) WHERE { ?s <urn:p>+ <urn:s0> }",
            "SELECT * WHERE { { ?s <urn:r> ?o } UNION { ?s <urn:sub> ?o } }",
            "SELECT * WHERE { GRAPH ?g { ?s <urn:sub> ?o } }",
            "SELECT ?g (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?s ?p ?o } } GROUP BY ?g ORDER BY ?g",
            "SELECT * WHERE { GRAPH <urn:ontomark:graph:asserted> { ?s <urn:r> ?o } }",
            "CONSTRUCT { ?o <urn:inv> ?s } WHERE { ?s <urn:r> ?o }",
            "SELECT ?s (CONCAT(STR(?s), \"!\") AS ?x) WHERE { ?s <urn:r> ?o } ORDER BY ?x",
        ];
        for sel in [
            DatasetSelection::Asserted,
            DatasetSelection::Inferred,
            DatasetSelection::Combined,
        ] {
            let reference = dataset(&asserted, &inferred, sel, true);
            let store = Store::build(&asserted, &inferred, sel, true);
            for q in queries {
                let parsed = parse_checked(q).unwrap();
                let mut want = evaluate(&parsed, &reference, 10_000).unwrap();
                let mut got = evaluate(&parsed, &store, 10_000).unwrap();
                for out in [&mut want, &mut got] {
                    match out {
                        QueryOutput::Select { rows, .. } => rows.sort(),
                        QueryOutput::Construct { triples, .. } => triples.sort(),
                        QueryOutput::Ask { .. } => {}
                    }
                }
                assert_eq!(got, want, "{sel:?}: {q}");
            }
        }
    }

    #[test]
    fn optional_union_construct_work() {
        let asserted = vec![
            t("urn:a", "urn:p", iri("urn:b")),
            t("urn:c", "urn:r", iri("urn:d")),
        ];
        let ds = dataset(&asserted, &[], DatasetSelection::Combined, false);
        let q = parse_checked("SELECT ?s ?o WHERE { { ?s <urn:p> ?o } UNION { ?s <urn:r> ?o } OPTIONAL { ?s <urn:none> ?x } }").unwrap();
        let QueryOutput::Select { rows, .. } = evaluate(&q, &ds, 10).unwrap() else {
            panic!()
        };
        assert_eq!(rows.len(), 2);
        let c = parse_checked("CONSTRUCT { ?o <urn:inv> ?s } WHERE { ?s <urn:p> ?o }").unwrap();
        let QueryOutput::Construct { triples, .. } = evaluate(&c, &ds, 10).unwrap() else {
            panic!()
        };
        assert_eq!(triples.len(), 1);
    }
}
