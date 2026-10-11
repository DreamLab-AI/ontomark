//! A compact, read-only quad store for `spareval`.
//!
//! [`oxrdf::Dataset`] keeps six B-tree indexes and is built one insert at a
//! time; on the estate corpus (≈370,000 triples) building it took seconds in
//! wasm for every dataset selection. This store is built once per generation
//! by dictionary-encoding terms to `u32` and sorting three permutations
//! (SPO, POS, OSP) in bulk, then answers triple patterns by binary-searched
//! prefix ranges. It implements `spareval`'s [`QueryableDataset`], so query
//! evaluation is still entirely Oxigraph's; only the storage is ours.
//!
//! Terms that appear only in a query or in computed expressions are interned
//! on demand (`internalize_term` takes `&self`), so the dictionary sits behind
//! a [`RefCell`]; the engine is single-threaded by design.

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;

use oxrdf::{NamedNode, Term, Triple};
use spareval::{InternalQuad, QueryableDataset};

use crate::query::{DatasetSelection, ASSERTED_GRAPH, INFERRED_GRAPH};

/// Graph id of the default graph.
const DEFAULT: u32 = u32::MAX;

#[derive(Default)]
struct Dictionary {
    terms: Vec<Term>,
    ids: HashMap<Term, u32>,
}

impl Dictionary {
    fn intern(&mut self, t: Term) -> u32 {
        if let Some(&id) = self.ids.get(&t) {
            return id;
        }
        let id = u32::try_from(self.terms.len()).expect("fewer than 2^32 - 1 distinct terms");
        assert!(id != DEFAULT, "term dictionary full");
        self.terms.push(t.clone());
        self.ids.insert(t, id);
        id
    }
}

/// A read-only quad store. Build with [`Store::build`].
pub struct Store {
    dict: RefCell<Dictionary>,
    /// Quads as `[s, p, o, g]`, sorted by (s, p, o, g).
    spo: Vec<[u32; 4]>,
    /// Same quads sorted by (p, o, s, g).
    pos: Vec<[u32; 4]>,
    /// Same quads sorted by (o, s, p, g).
    osp: Vec<[u32; 4]>,
}

impl Store {
    /// Build the query dataset: `selection` as the default graph, plus the
    /// asserted and inferred named graphs when `named_graphs` is set.
    #[must_use]
    pub fn build(
        asserted: &[Triple],
        inferred: &[Triple],
        selection: DatasetSelection,
        named_graphs: bool,
    ) -> Self {
        let mut dict = Dictionary::default();
        let mut quads: Vec<[u32; 4]> = Vec::new();
        let mut add = |dict: &mut Dictionary, t: &Triple, g: u32| {
            let s = dict.intern(t.subject.clone().into());
            let p = dict.intern(t.predicate.clone().into());
            let o = dict.intern(t.object.clone());
            quads.push([s, p, o, g]);
        };
        let graph_id =
            |dict: &mut Dictionary, iri: &str| dict.intern(NamedNode::new_unchecked(iri).into());
        let (ga, gi) = if named_graphs {
            (
                graph_id(&mut dict, ASSERTED_GRAPH),
                graph_id(&mut dict, INFERRED_GRAPH),
            )
        } else {
            (DEFAULT, DEFAULT)
        };
        for t in asserted {
            if selection != DatasetSelection::Inferred {
                add(&mut dict, t, DEFAULT);
            }
            if named_graphs {
                add(&mut dict, t, ga);
            }
        }
        for t in inferred {
            if selection != DatasetSelection::Asserted {
                add(&mut dict, t, DEFAULT);
            }
            if named_graphs {
                add(&mut dict, t, gi);
            }
        }
        quads.sort_unstable();
        quads.dedup();
        let mut pos: Vec<[u32; 4]> = quads.iter().map(|&[s, p, o, g]| [p, o, s, g]).collect();
        pos.sort_unstable();
        let mut osp: Vec<[u32; 4]> = quads.iter().map(|&[s, p, o, g]| [o, s, p, g]).collect();
        osp.sort_unstable();
        Self {
            dict: RefCell::new(dict),
            spo: quads,
            pos,
            osp,
        }
    }

    /// Number of distinct quads.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spo.len()
    }

    /// `true` when the store holds no quads.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spo.is_empty()
    }
}

/// The sub-slice of `index` whose first `prefix.len()` positions equal `prefix`.
fn range<'s>(index: &'s [[u32; 4]], prefix: &[u32]) -> &'s [[u32; 4]] {
    let n = prefix.len();
    let lo = index.partition_point(|q| q[..n] < *prefix);
    let hi = index.partition_point(|q| q[..n] <= *prefix);
    &index[lo..hi]
}

impl<'a> QueryableDataset<'a> for &'a Store {
    type InternalTerm = u32;
    type Error = Infallible;

    fn internal_quads_for_pattern(
        &self,
        subject: Option<&u32>,
        predicate: Option<&u32>,
        object: Option<&u32>,
        graph_name: Option<Option<&u32>>,
    ) -> impl Iterator<Item = Result<InternalQuad<u32>, Infallible>> + use<'a> {
        let store: &'a Store = self;
        let (s, p, o) = (subject.copied(), predicate.copied(), object.copied());
        // Pick the permutation whose prefix covers the most bound positions.
        // `order` maps the permutation's columns back to (s, p, o, g).
        let (slice, order): (&'a [[u32; 4]], [usize; 4]) = match (s, p, o) {
            (Some(s), Some(p), Some(o)) => (range(&store.spo, &[s, p, o]), [0, 1, 2, 3]),
            (Some(s), Some(p), None) => (range(&store.spo, &[s, p]), [0, 1, 2, 3]),
            (Some(s), None, Some(o)) => (range(&store.osp, &[o, s]), [1, 2, 0, 3]),
            (Some(s), None, None) => (range(&store.spo, &[s]), [0, 1, 2, 3]),
            (None, Some(p), Some(o)) => (range(&store.pos, &[p, o]), [2, 0, 1, 3]),
            (None, Some(p), None) => (range(&store.pos, &[p]), [2, 0, 1, 3]),
            (None, None, Some(o)) => (range(&store.osp, &[o]), [1, 2, 0, 3]),
            (None, None, None) => (&store.spo[..], [0, 1, 2, 3]),
        };
        let graph = graph_name.map(|g| g.copied());
        slice.iter().filter_map(move |q| {
            let quad = [q[order[0]], q[order[1]], q[order[2]], q[order[3]]];
            let [qs, qp, qo, qg] = quad;
            let graph_ok = match graph {
                None => qg != DEFAULT,
                Some(None) => qg == DEFAULT,
                Some(Some(g)) => qg == g,
            };
            let ok = graph_ok
                && s.is_none_or(|x| x == qs)
                && p.is_none_or(|x| x == qp)
                && o.is_none_or(|x| x == qo);
            ok.then_some(Ok(InternalQuad {
                subject: qs,
                predicate: qp,
                object: qo,
                graph_name: (qg != DEFAULT).then_some(qg),
            }))
        })
    }

    fn internalize_term(&self, term: Term) -> Result<u32, Infallible> {
        Ok(self.dict.borrow_mut().intern(term))
    }

    fn externalize_term(&self, term: u32) -> Result<Term, Infallible> {
        Ok(self.dict.borrow().terms[term as usize].clone())
    }
}
