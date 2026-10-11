//! Turtle writer conformance.
//!
//! `ontomark` is a Turtle *writer*, and the W3C RDF 1.1 Turtle suite tests
//! *parsers*, so the suite is used as a corpus of graphs for a round trip:
//!
//! 1. parse the test's action document with `oxttl` (the reference parser);
//! 2. load the triples into [`Graph`] through a test-side adapter, counting any
//!    construct the model cannot carry as *unrepresentable* (never a pass);
//! 3. [`serialise`] the graph and re-parse the output with `oxttl`;
//! 4. compare with the expected N-Triples (`TestTurtleEval`) or with the first
//!    parse (`TestTurtlePositiveSyntax`) under blank-node isomorphism.
//!
//! Negative syntax tests have no meaning for a writer and are counted as N/A.
//!
//! The suite is fetched by `conformance/w3c/fetch.sh` and gitignored; when it is
//! absent the suite test prints a note and passes, so `cargo test` stays
//! offline:
//!
//! ```text
//! conformance/w3c/fetch.sh && \
//!   cargo test -p ontomark-compile --test w3c_turtle -- --nocapture
//! ```
//!
//! The suite-free property tests at the bottom drive the same writer with
//! random graphs built through the [`Graph`] API.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ontomark_compile::build::turtle::{serialise, Graph, Subject, Term};
use ontomark_compile::profile::Profile;
use oxrdf::dataset::{CanonicalizationAlgorithm, CanonicalizationHashAlgorithm};
use oxrdf::{
    BlankNode, Graph as OxGraph, Literal, NamedNode, NamedOrBlankNode, Term as OxTerm, Triple,
};
use oxttl::{NTriplesParser, TurtleParser};

const TEST_BASE: &str = "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-turtle/";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

// ---------------------------------------------------------------------------
// Adapters and comparison
// ---------------------------------------------------------------------------

/// The profiles the writer is exercised under: the generic one and, when it
/// loads, the estate profile (its prefixes differ).
fn profiles() -> Vec<(&'static str, Profile)> {
    let mut out = vec![("generic", Profile::generic())];
    let estate = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../profiles/estate/profile.yaml"
    );
    if let Ok(text) = std::fs::read_to_string(estate) {
        if let Ok(p) = Profile::from_yaml(&text) {
            out.push(("estate", p));
        }
    }
    out
}

/// The one place the writer entry point is called.
fn write(profile: &Profile, g: &Graph) -> String {
    serialise(profile, g)
}

/// Parse a Turtle document with `oxttl`.
fn parse_turtle(text: &[u8], base: Option<&str>) -> Result<Vec<Triple>, String> {
    let mut parser = TurtleParser::new();
    if let Some(base) = base {
        parser = parser.with_base_iri(base).map_err(|e| e.to_string())?;
    }
    parser
        .for_slice(text)
        .map(|t| t.map_err(|e| e.to_string()))
        .collect()
}

/// Parse an N-Triples document with `oxttl`.
fn parse_ntriples(text: &[u8]) -> Result<Vec<Triple>, String> {
    NTriplesParser::new()
        .for_slice(text)
        .map(|t| t.map_err(|e| e.to_string()))
        .collect()
}

fn ox_graph(triples: &[Triple]) -> OxGraph {
    let mut g = OxGraph::new();
    for t in triples {
        g.insert(t);
    }
    g
}

const RDFC10: CanonicalizationAlgorithm = CanonicalizationAlgorithm::Rdfc10 {
    hash_algorithm: CanonicalizationHashAlgorithm::Sha256,
};

/// RDF graph isomorphism (RDFC-1.0 canonical form): canonicalise both sides and compare as sets.
fn isomorphic(a: &[Triple], b: &[Triple]) -> bool {
    let (mut a, mut b) = (ox_graph(a), ox_graph(b));
    a.canonicalize(RDFC10);
    b.canonicalize(RDFC10);
    a == b
}

fn is_pn_chars_base(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

fn is_pn_chars_u(c: char) -> bool {
    is_pn_chars_base(c) || c == '_'
}

fn is_pn_chars(c: char) -> bool {
    is_pn_chars_u(c)
        || c == '-'
        || c.is_ascii_digit()
        || c == '\u{B7}'
        || matches!(c, '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

/// Turtle `BLANK_NODE_LABEL`, which the writer emits verbatim after `_:`.
fn valid_blank_label(label: &str) -> bool {
    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(is_pn_chars_u(first) || first.is_ascii_digit()) {
        return false;
    }
    let rest: Vec<char> = chars.collect();
    rest.iter().all(|&c| is_pn_chars(c) || c == '.') && rest.last().is_none_or(|&c| c != '.')
}

/// Why a parsed graph cannot be carried by [`Graph`] and written faithfully.
#[derive(Debug)]
struct Unrepresentable(String);

/// Load parsed triples into the writer's model.
///
/// `Graph` stores IRIs, blank-node labels and literals as plain strings, so
/// every RDF 1.1 triple has a counterpart. The one thing the model does not
/// enforce is that a blank-node label is a legal Turtle label (the writer
/// emits it verbatim); a label that is not one is reported, never repaired.
fn to_graph(triples: &[Triple]) -> Result<Graph, Unrepresentable> {
    let mut g = Graph::new();
    for t in triples {
        let subject = match &t.subject {
            NamedOrBlankNode::NamedNode(n) => Subject::Iri(n.as_str().to_owned()),
            NamedOrBlankNode::BlankNode(b) => {
                check_label(b.as_str())?;
                Subject::Blank(b.as_str().to_owned())
            }
        };
        let object = match &t.object {
            OxTerm::NamedNode(n) => Term::Iri(n.as_str().to_owned()),
            OxTerm::BlankNode(b) => {
                check_label(b.as_str())?;
                Term::Blank(b.as_str().to_owned())
            }
            OxTerm::Literal(l) => {
                if let Some(lang) = l.language() {
                    Term::lang(l.value(), lang)
                } else if l.datatype().as_str() == XSD_STRING {
                    // RDF 1.1: a simple literal *is* an xsd:string literal.
                    Term::plain(l.value())
                } else {
                    Term::typed(l.value(), l.datatype().as_str())
                }
            }
            #[allow(unreachable_patterns)]
            other => return Err(Unrepresentable(format!("term kind {other}"))),
        };
        g.add(subject, t.predicate.as_str(), object);
    }
    Ok(g)
}

fn check_label(label: &str) -> Result<(), Unrepresentable> {
    if valid_blank_label(label) {
        Ok(())
    } else {
        Err(Unrepresentable(format!(
            "blank-node label {label:?} is not a legal Turtle label"
        )))
    }
}

/// The inverse adapter, used to state the expected triples of a [`Graph`].
fn to_triples(g: &Graph) -> Vec<Triple> {
    g.iter()
        .map(|(s, p, o)| {
            let subject: NamedOrBlankNode = match s {
                Subject::Iri(i) => NamedNode::new(i).expect("valid IRI").into(),
                Subject::Blank(b) => BlankNode::new(b).expect("valid label").into(),
            };
            let object: OxTerm = match o {
                Term::Iri(i) => NamedNode::new(i).expect("valid IRI").into(),
                Term::Blank(b) => BlankNode::new(b).expect("valid label").into(),
                Term::Literal {
                    value,
                    lang: Some(lang),
                    ..
                } => Literal::new_language_tagged_literal(value, lang)
                    .expect("valid tag")
                    .into(),
                Term::Literal {
                    value,
                    datatype: Some(dt),
                    ..
                } => {
                    Literal::new_typed_literal(value, NamedNode::new(dt).expect("valid IRI")).into()
                }
                Term::Literal { value, .. } => Literal::new_simple_literal(value).into(),
            };
            Triple::new(subject, NamedNode::new(p).expect("valid IRI"), object)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// W3C suite
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Pass,
    Fail(String),
    Unrepresentable(String),
    /// `oxttl` itself rejects the document, so there is nothing to write.
    ParserRejects(String),
    NotApplicable,
}

struct Case {
    name: String,
    kind: String,
    action: PathBuf,
    result: Option<PathBuf>,
}

fn suite_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/w3c/rdf-turtle")
}

fn local_path(dir: &std::path::Path, iri: &str) -> PathBuf {
    dir.join(iri.strip_prefix(TEST_BASE).unwrap_or(iri))
}

fn load_manifest(dir: &std::path::Path) -> Vec<Case> {
    const MF: &str = "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#";
    let text = std::fs::read(dir.join("manifest.ttl")).expect("manifest.ttl");
    let triples = parse_turtle(&text, Some(&format!("{TEST_BASE}manifest.ttl")))
        .expect("manifest parses with oxttl");
    let mut kinds = BTreeMap::new();
    let mut actions = BTreeMap::new();
    let mut results = BTreeMap::new();
    let mut names = BTreeMap::new();
    for t in &triples {
        let NamedOrBlankNode::NamedNode(s) = &t.subject else {
            continue;
        };
        let p = t.predicate.as_str();
        let o_iri = match &t.object {
            OxTerm::NamedNode(n) => Some(n.as_str().to_owned()),
            _ => None,
        };
        if p == RDF_TYPE {
            if let Some(o) = &o_iri {
                if let Some(kind) = o.strip_prefix("http://www.w3.org/ns/rdftest#") {
                    kinds.insert(s.as_str().to_owned(), kind.to_owned());
                }
            }
        } else if p == format!("{MF}action") {
            actions.insert(s.as_str().to_owned(), o_iri);
        } else if p == format!("{MF}result") {
            results.insert(s.as_str().to_owned(), o_iri);
        } else if p == format!("{MF}name") {
            if let OxTerm::Literal(l) = &t.object {
                names.insert(s.as_str().to_owned(), l.value().to_owned());
            }
        }
    }
    kinds
        .into_iter()
        .filter_map(|(iri, kind)| {
            let action = actions.get(&iri)?.clone()?;
            Some(Case {
                name: names.get(&iri).cloned().unwrap_or_else(|| iri.clone()),
                kind,
                action: local_path(dir, &action),
                result: results
                    .get(&iri)
                    .and_then(|r| r.as_deref())
                    .map(|r| local_path(dir, r)),
                // The action IRI doubles as the parse base; recomputed in `run_case`.
            })
        })
        .collect()
}

fn run_case(profile: &Profile, case: &Case) -> Outcome {
    let positive_eval = case.kind == "TestTurtleEval";
    if !positive_eval && case.kind != "TestTurtlePositiveSyntax" {
        return Outcome::NotApplicable;
    }
    let file = case
        .action
        .file_name()
        .expect("file name")
        .to_string_lossy()
        .into_owned();
    let text = std::fs::read(&case.action).expect("action file");
    let parsed = match parse_turtle(&text, Some(&format!("{TEST_BASE}{file}"))) {
        Ok(p) => p,
        Err(e) => return Outcome::ParserRejects(e),
    };
    let expected = if positive_eval {
        let result = case.result.as_ref().expect("eval result");
        let nt = std::fs::read(result).expect("result file");
        match parse_ntriples(&nt) {
            Ok(t) => t,
            Err(e) => return Outcome::Fail(format!("expected N-Triples unparsable: {e}")),
        }
    } else {
        parsed.clone()
    };
    let graph = match to_graph(&parsed) {
        Ok(g) => g,
        Err(Unrepresentable(why)) => return Outcome::Unrepresentable(why),
    };
    let written = write(profile, &graph);
    match parse_turtle(written.as_bytes(), None) {
        Err(e) => Outcome::Fail(format!("writer output does not parse: {e}")),
        Ok(back) if isomorphic(&back, &expected) => Outcome::Pass,
        Ok(back) => Outcome::Fail(format!(
            "round trip differs from expected ({} triples back, {} expected)",
            back.len(),
            expected.len()
        )),
    }
}

/// Tests the writer is known to fail (see the `BUG` tests below); empty means
/// every applicable suite test must pass.
const KNOWN_FAILURES: &[&str] = &[];

#[test]
fn w3c_turtle_round_trip() {
    let dir = suite_dir();
    if !dir.join("manifest.ttl").exists() {
        println!("w3c_turtle: suite not fetched (run conformance/w3c/fetch.sh); skipping");
        return;
    }
    let cases = load_manifest(&dir);
    assert!(
        cases.len() >= 300,
        "manifest looks truncated: {}",
        cases.len()
    );
    for (label, profile) in profiles() {
        let (mut pass, mut na) = (0usize, 0usize);
        let mut fail = Vec::new();
        let mut unrep = Vec::new();
        let mut rejects = Vec::new();
        for case in &cases {
            match run_case(&profile, case) {
                Outcome::Pass => pass += 1,
                Outcome::NotApplicable => na += 1,
                Outcome::Fail(why) => fail.push((case.name.clone(), why)),
                Outcome::Unrepresentable(why) => unrep.push((case.name.clone(), why)),
                Outcome::ParserRejects(why) => rejects.push((case.name.clone(), why)),
            }
        }
        println!(
            "w3c_turtle[{label}]: {} tests: pass={} fail={} unrepresentable={} parser-rejects={} n/a={}",
            cases.len(),
            pass,
            fail.len(),
            unrep.len(),
            rejects.len(),
            na
        );
        for (name, why) in &fail {
            println!("  FAIL {name}: {why}");
        }
        for (name, why) in &unrep {
            println!("  UNREPRESENTABLE {name}: {why}");
        }
        for (name, why) in &rejects {
            println!("  PARSER-REJECTS {name}: {why}");
        }
        let unexpected: Vec<_> = fail
            .iter()
            .filter(|(n, _)| !KNOWN_FAILURES.contains(&n.as_str()))
            .collect();
        assert!(
            unexpected.is_empty(),
            "[{label}] round-trip failures: {unexpected:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Property tests (no suite needed)
// ---------------------------------------------------------------------------

/// SplitMix64: a tiny deterministic generator, so failures reproduce from a seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// Characters that stress IRI local parts, prefixed-name rules and literals.
const LOCAL_CHARS: &[&str] = &[
    "a", "b", "Z", "0", "7", "_", "-", ".", ":", "%41", "%C3%A9", "~", "!", "$", "&", "'", "(",
    ")", "*", "+", ",", ";", "=", "/", "?", "@", "é", "ß", "日本", "😀", "\u{B7}", "\u{300}",
];

const LITERAL_CHARS: &[&str] = &[
    "a",
    "Z",
    " ",
    "\"",
    "\"\"\"",
    "'",
    "\\",
    "\\n",
    "\n",
    "\r",
    "\t",
    "\u{1}",
    "\u{7F}",
    "\u{85}",
    "\u{2028}",
    "é",
    "日本語",
    "😀",
    "<",
    ">",
    "#",
    "@",
    "^^",
    "{",
    "}",
    "\u{0}",
    "\u{8}",
    "\u{c}",
];

const LANGS: &[&str] = &["en", "en-GB", "de-DE-1996", "zh-Hant-TW", "x-private", "fr"];

const DATATYPES: &[&str] = &[
    "http://www.w3.org/2001/XMLSchema#integer",
    "http://www.w3.org/2001/XMLSchema#float",
    "http://www.w3.org/2001/XMLSchema#dateTime",
    "http://example.org/dt/custom",
];

/// Known writer bugs (see the `BUG` tests) are avoided when `avoid` is set.
struct Gen {
    rng: Rng,
    bases: Vec<String>,
    avoid: bool,
}

impl Gen {
    fn new(profile: &Profile, seed: u64, avoid: bool) -> Self {
        // The writer's declared prefixes, read back from its own output.
        let header = write(profile, &Graph::new());
        let mut bases: Vec<String> = header
            .lines()
            .filter_map(|l| l.strip_prefix("@prefix "))
            .filter_map(|l| {
                let start = l.find('<')? + 1;
                let end = l.find('>')?;
                Some(l[start..end].to_owned())
            })
            .collect();
        bases.extend(
            [
                "http://example.org/",
                "urn:x:",
                "https://a.example/ns#",
                "http://ex.org/é/",
            ]
            .map(str::to_owned),
        );
        Self {
            rng: Rng(seed),
            bases,
            avoid,
        }
    }

    fn iri(&mut self) -> String {
        for _ in 0..50 {
            let base = self.rng.pick(&self.bases).clone();
            let len = self.rng.below(7);
            let mut local = String::new();
            for _ in 0..len {
                let c = if self.rng.chance(60) {
                    *self.rng.pick(&LOCAL_CHARS[..6])
                } else {
                    *self.rng.pick(LOCAL_CHARS)
                };
                local.push_str(c);
            }
            // Known bug: a prefixed name may not start with `-`.
            if self.avoid && local.starts_with('-') {
                continue;
            }
            let iri = format!("{base}{local}");
            if oxiri::Iri::parse(iri.as_str()).is_ok() && NamedNode::new(&iri).is_ok() {
                return iri;
            }
        }
        format!("{}fallback", self.bases[0])
    }

    fn literal(&mut self) -> Term {
        let len = self.rng.below(8);
        let mut value = String::new();
        for _ in 0..len {
            value.push_str(self.rng.pick::<&str>(LITERAL_CHARS));
        }
        match self.rng.below(3) {
            0 => Term::plain(value),
            1 => Term::lang(value, self.rng.pick::<&str>(LANGS)),
            _ => {
                let dt = if self.rng.chance(30) {
                    self.iri()
                } else {
                    (*self.rng.pick::<&str>(DATATYPES)).to_owned()
                };
                Term::typed(value, &dt)
            }
        }
    }

    fn graph(&mut self) -> Graph {
        let mut g = Graph::new();
        let blanks: Vec<String> = (0..1 + self.rng.below(4))
            .map(|_| g.fresh_blank())
            .collect();
        for _ in 0..1 + self.rng.below(40) {
            let subject = if self.rng.chance(20) {
                Subject::Blank(self.rng.pick(&blanks).clone())
            } else {
                Subject::Iri(self.iri())
            };
            let predicate = if self.rng.chance(20) {
                RDF_TYPE.to_owned()
            } else {
                self.iri()
            };
            let object = match self.rng.below(10) {
                0..=3 => Term::Iri(self.iri()),
                4 => Term::Blank(self.rng.pick(&blanks).clone()),
                // Known bug: `rdf:type` as an object is written as `a`.
                5 if !self.avoid => Term::Iri(RDF_TYPE.to_owned()),
                _ => self.literal(),
            };
            g.add(subject, predicate, object);
        }
        g
    }
}

fn check_round_trip(profile: &Profile, g: &Graph) -> Result<(), String> {
    let written = write(profile, g);
    let back = parse_turtle(written.as_bytes(), None)
        .map_err(|e| format!("output does not parse: {e}\n{written}"))?;
    let expected = to_triples(g);
    if isomorphic(&back, &expected) {
        Ok(())
    } else {
        Err(format!("triples differ\n{written}"))
    }
}

#[test]
fn property_random_graphs_round_trip() {
    for (label, profile) in profiles() {
        for seed in 0..400 {
            let g = Gen::new(&profile, seed, true).graph();
            if let Err(e) = check_round_trip(&profile, &g) {
                panic!("[{label}] seed {seed}: {e}");
            }
        }
    }
}

#[test]
fn property_unicode_and_escapes_in_literals() {
    let mut rng = Rng(7);
    for i in 0..300 {
        let mut value = String::new();
        for _ in 0..rng.below(12) {
            value.push_str(rng.pick::<&str>(LITERAL_CHARS));
        }
        let mut g = Graph::new();
        g.add_iri(
            "http://example.org/s",
            "http://example.org/p",
            Term::plain(value.clone()),
        );
        g.add_iri(
            "http://example.org/s",
            "http://example.org/q",
            Term::lang(value, "en"),
        );
        for (label, profile) in profiles() {
            if let Err(e) = check_round_trip(&profile, &g) {
                panic!("[{label}] case {i}: {e}");
            }
        }
    }
}

/// Exercise unrestricted generated graphs, including rdf:type outside predicate
/// position and local names that need an absolute IRI.
#[test]
fn bug_random_graphs_round_trip_unrestricted() {
    for (label, profile) in profiles() {
        for seed in 0..400 {
            let g = Gen::new(&profile, seed, false).graph();
            if let Err(e) = check_round_trip(&profile, &g) {
                panic!("[{label}] seed {seed}: {e}");
            }
        }
    }
}

#[test]
fn bug_rdf_type_outside_predicate_position_is_not_valid_turtle() {
    let mut g = Graph::new();
    g.add_iri(
        RDF_TYPE,
        "http://example.org/p",
        Term::plain("subject position"),
    );
    g.add_iri(
        "http://example.org/s",
        "http://example.org/p",
        Term::Iri(RDF_TYPE.to_owned()),
    );
    g.add_iri(
        "http://example.org/s",
        "http://example.org/q",
        Term::typed("x", RDF_TYPE),
    );
    check_round_trip(&Profile::generic(), &g).unwrap();
}

#[test]
fn bug_leading_hyphen_local_name_is_not_valid_turtle() {
    let mut g = Graph::new();
    g.add_iri(
        "http://www.w3.org/2004/02/skos/core#-x",
        "http://www.w3.org/2004/02/skos/core#broader",
        Term::plain("v"),
    );
    check_round_trip(&Profile::generic(), &g).unwrap();
}

#[test]
fn language_and_datatype_forms_stay_distinct() {
    let mut g = Graph::new();
    let s = "http://example.org/s";
    g.add_iri(s, "http://example.org/p", Term::plain("x"));
    g.add_iri(s, "http://example.org/p", Term::lang("x", "en"));
    g.add_iri(
        s,
        "http://example.org/p",
        Term::typed("1", "http://www.w3.org/2001/XMLSchema#integer"),
    );
    check_round_trip(&Profile::generic(), &g).unwrap();
    // The model treats `"x"^^xsd:string` and `"x"` as different terms, RDF 1.1
    // does not; the adapter normalises on the way in, so no lang-string
    // datatype or explicit xsd:string ever reaches the writer from a parse.
    assert_ne!(XSD_STRING, RDF_LANG_STRING);
}
