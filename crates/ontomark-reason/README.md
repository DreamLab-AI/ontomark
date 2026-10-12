# ontomark-reason

OWL 2 EL reasoning for ontomark: a profile check that refuses what EL cannot
express (and says why), and a classification facade over
[`whelk-el`](https://crates.io/crates/whelk-el) that reports inferred
subsumptions, unsatisfiable classes, their root causes and the disjointness
axioms behind them.

Input is logical axioms, never Markdown; nothing here depends on the
parser or the model, and it performs no I/O. It runs unchanged on
`wasm32-unknown-unknown`, which is how the same reasoner serves an Obsidian
worker and a server.

## Why EL, and why refusals are a feature

OWL 2 EL is the profile in which subsumption stays polynomial even with general
concept inclusions. That is what lets a vault of ten thousand notes classify
in seconds in a browser, and it is the profile the whole DreamLab estate
reasons in. The price is expressiveness: universal restrictions, cardinalities,
unions, inverse properties and nominals are outside it.

This crate's position is that an axiom it cannot reason over must be **named**,
not dropped. Every refusal carries the construct and the axiom; a result with
any refusal reports `is_complete() == false`. A caller can therefore tell the
difference between "nothing follows" and "I could not look", which no silently
incomplete reasoner allows.

```rust
use ontomark_reason::axiom::{Axiom, ClassExpression as C, Property};
use ontomark_reason::classify;

let axioms = vec![
    Axiom::sub_class("urn:P", "urn:A"),
    Axiom::sub_class("urn:P", "urn:B"),
    Axiom::DisjointClasses(vec![C::class("urn:A"), C::class("urn:B")]),
    // A universal restriction: outside EL.
    Axiom::SubClassOf {
        sub: C::class("urn:Q"),
        sup: C::Only { property: Property::named("urn:r"), filler: Box::new(C::class("urn:A")) },
    },
];
let result = classify(&axioms);

// P is a subclass of two disjoint classes: unsatisfiable, with the cause named.
assert_eq!(result.unsatisfiable, vec!["urn:P".to_owned()]);
assert_eq!(result.disjointness_violations[0].disjoint, ("urn:A".to_owned(), "urn:B".to_owned()));
// The universal restriction was refused, by name, and the result says it is incomplete.
assert_eq!(result.refusals[0].construct, "ObjectAllValuesFrom");
assert!(!result.is_complete());
```

## What is supported

Named classes, intersections, existential restrictions (`some`), class and
property hierarchies, disjointness, domains, and individual type assertions
expanded through the class hierarchy. Ranges, built-in properties, nominals,
data properties and individual-level reasoning beyond typing are reported as
refusals. The supported fragment is documented on the `axiom` module and
tested; "no refusals" establishes completeness for that fragment only, and
the crate makes no claim of complete OWL 2 EL.

## Sessions for responsive hosts

`classify` runs to completion. `Session` runs the same saturation in bounded
steps so a host can yield between them, report progress, and cancel: a vault
edit in Obsidian supersedes the running classification instead of blocking
behind it. Both paths produce identical results.

## Features

- `corpus-bench` (native only): a benchmark over a committed corpus fixture.

Version 0.1.1 requires `whelk-el` 0.1.1 or later, which carries the corrected
upstream attribution and the Scala Whelk BSD-3-Clause notice. Reasoning
behaviour is unchanged from 0.1.0.

Licensed under MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
