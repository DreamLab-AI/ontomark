# ontomark-reason

Conservative supported-profile checking and classification over `whelk-el`.
The crate accepts logical axioms with string IRIs and reports named subsumptions,
unsatisfiable classes, root causes and refused constructs. It performs no I/O.

```rust
use ontomark_reason::{axiom::Axiom, classify};
let result = classify(&[
    Axiom::sub_class("urn:A", "urn:B"),
    Axiom::sub_class("urn:B", "urn:C"),
]);
assert!(result.inferred.contains(&("urn:A".into(), "urn:C".into())));
```

This is not a claim of complete OWL 2 EL support. Profile refusals and deliberately
dropped existential restrictions make a result incomplete. Even a result with
no refusals establishes completeness only for the crate's documented supported
fragment. Unsupported nominals, ranges and other constructs are reported rather
than silently treated as ordinary edges. Bounded step-based sessions support
responsive hosts; optional `corpus-bench` enables a native-only benchmark.

Version 0.1.1 requires `whelk-el` 0.1.1 or later, which carries the corrected
upstream attribution and Scala BSD notice. Reasoning behaviour is unchanged.

Licensed under MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
