// whelk-el is derived from whelk-rs (https://github.com/jjohare/whelk-rs, rev
// 79a1ee2), itself a Rust port of Whelk (https://github.com/balhoff/whelk).
// Copyright (c) 2022 The Authors (Jim Balhoff and the whelk-rs contributors),
// MIT licence: see LICENSE-WHELK and NOTICE. Changes in this crate are under
// MIT OR Apache-2.0.

//! An OWL 2 EL reasoner: Whelk's saturation rules, portable to
//! `wasm32-unknown-unknown`, with no I/O, no clocks and no threads in the
//! default build.
//!
//! The crate takes [`ElAxiom`]s (owned, string IRIs), normalises them into the
//! reasoner's concept language and saturates. The result is every entailed
//! subsumption between named classes, including those to `owl:Nothing`, which
//! mark unsatisfiable classes.
//!
//! ```
//! use whelk_el::{classify, ClassExpr, ElAxiom};
//!
//! let axioms = vec![
//!     ElAxiom::sub_class("urn:Arm", "urn:Robot"),
//!     ElAxiom::SubClassOf {
//!         sub: ClassExpr::named("urn:Arm"),
//!         sup: ClassExpr::some("urn:hasPart", ClassExpr::named("urn:Gripper")),
//!     },
//!     // GrippingRobot ≡ Robot ⊓ ∃hasPart.Gripper
//!     ElAxiom::EquivalentClasses(vec![
//!         ClassExpr::named("urn:GrippingRobot"),
//!         ClassExpr::And(vec![
//!             ClassExpr::named("urn:Robot"),
//!             ClassExpr::some("urn:hasPart", ClassExpr::named("urn:Gripper")),
//!         ]),
//!     ]),
//! ];
//! let entailed = classify(&axioms);
//! assert!(entailed.contains(&("urn:Arm".to_owned(), "urn:GrippingRobot".to_owned())));
//! ```
//!
//! # Long-running work
//!
//! [`reasoner::Saturation`] runs the same computation in bounded slices of
//! steps, so a host (a web worker, an editor) can interleave reasoning with
//! its own events and abandon a stale run. Time budgets are the host's
//! business; this crate counts steps only.
//!
//! # Features
//!
//! * `parallel` (off by default): [`classify_parallel`] splits the input into
//!   independent components and saturates them on the `rayon` pool. Native
//!   only in practice; the result is identical to [`classify`].
//! * `horned-owl` (off by default): [`owl`] converts `horned-owl` components
//!   into [`ElAxiom`]s. `horned-owl` does not build for
//!   `wasm32-unknown-unknown` (its RDF stack requires `getrandom`), which is
//!   why the core input type is this crate's own.
//!
//! # What is supported
//!
//! Named classes, `owl:Thing`, `owl:Nothing`, intersections, existential
//! restrictions, singleton nominals; sub-, equivalent, disjoint classes; sub-
//! and equivalent properties, property chains, transitivity, domains; class
//! and property assertions. Property ranges, reflexivity, self restrictions
//! and data properties are not implemented, and have no constructor here.

pub mod input;
pub mod model;
#[cfg(feature = "horned-owl")]
pub mod owl;
pub mod reasoner;

pub use input::{translate, ClassExpr, ElAxiom};
pub use model::{BOTTOM, TOP};
pub use reasoner::{assert, assert_append, Progress, ReasonerState, Saturation};

/// Every entailed subsumption between named classes, as sorted,
/// deduplicated `(sub, sup)` IRI pairs.
///
/// Includes reflexive pairs, pairs to `owl:Thing`, the pairs `(C, owl:Nothing)`
/// for every unsatisfiable class `C` and the pairs `(owl:Nothing, C)` for every
/// class the reasoner saw.
#[must_use]
pub fn classify(axioms: &[ElAxiom]) -> Vec<(String, String)> {
    assert(&translate(axioms)).named_subsumption_ids()
}

/// Partition `axioms` into groups that share no entity (other than
/// `owl:Thing` and `owl:Nothing`), as lists of indexes into `axioms`, in order.
///
/// Saturating each group separately entails exactly what saturating all of
/// them entails: EL rules only combine premises that share a concept or role.
/// The one exception is an axiom that puts `owl:Thing` on the left
/// ([`ElAxiom::constrains_everything`]), which reaches every class; its
/// presence yields a single group.
#[must_use]
pub fn components(axioms: &[ElAxiom]) -> Vec<Vec<usize>> {
    if axioms.iter().any(ElAxiom::constrains_everything) {
        return vec![(0..axioms.len()).collect()];
    }
    let mut ids: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    let mut first_entity: Vec<Option<usize>> = Vec::with_capacity(axioms.len());
    for axiom in axioms {
        let mut first: Option<usize> = None;
        let mut entities = Vec::new();
        axiom.for_each_entity(&mut |iri| entities.push(iri));
        for iri in entities {
            let next = parent.len();
            let id = *ids.entry(iri).or_insert_with(|| {
                parent.push(next);
                next
            });
            match first {
                None => first = Some(id),
                Some(f) => union(&mut parent, f, id),
            }
        }
        first_entity.push(first);
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of_root: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    let mut entity_free: Vec<usize> = Vec::new();
    for (index, first) in first_entity.into_iter().enumerate() {
        match first {
            None => entity_free.push(index),
            Some(entity) => {
                let root = find(&mut parent, entity);
                let group = *group_of_root.entry(root).or_insert_with(|| {
                    groups.push(Vec::new());
                    groups.len() - 1
                });
                groups[group].push(index);
            }
        }
    }
    if !entity_free.is_empty() {
        groups.push(entity_free);
    }
    groups
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        parent[ra.max(rb)] = ra.min(rb);
    }
}

/// [`classify`], with independent [`components`] saturated in parallel on the
/// `rayon` pool. The result is identical to [`classify`].
#[cfg(feature = "parallel")]
#[must_use]
pub fn classify_parallel(axioms: &[ElAxiom]) -> Vec<(String, String)> {
    use rayon::prelude::*;
    let groups = components(axioms);
    let mut pairs: Vec<(String, String)> = groups
        .par_iter()
        .map(|group| {
            let subset: Vec<ElAxiom> = group.iter().map(|&i| axioms[i].clone()).collect();
            classify(&subset)
        })
        .flatten()
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}
