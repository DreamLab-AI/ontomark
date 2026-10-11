# whelk-el

Portable saturation engine derived from Jim Balhoff's Whelk and its Rust port,
with owned string IRIs and no I/O. The default build uses no threads; bounded
step-based sessions let a host interleave classification with its event loop.

```rust
use whelk_el::{classify, ElAxiom};
let entailed = classify(&[
    ElAxiom::sub_class("urn:A", "urn:B"),
    ElAxiom::sub_class("urn:B", "urn:C"),
]);
assert!(entailed.contains(&("urn:A".into(), "urn:C".into())));
```

The input supports named classes, top/bottom, intersections and existential
restrictions, class axioms, property hierarchies/chains, transitivity, domains
and assertions. Ranges, reflexivity, self restrictions and data properties
are not implemented. Nominals lack merging, so nominal consequences are
incomplete. For a conservative supported-profile check and refusal reporting,
use `ontomark-reason` rather than treating this engine as a complete OWL 2 EL
implementation.

Features: `parallel` partitions independent components on Rayon; `horned-owl`
adds a native-only adapter. Neither feature is required for the portable core.

The Scala Whelk upstream, by Jim Balhoff and contributors, uses BSD-3-Clause;
its notice is preserved in `LICENSE-WHELK-BSD`. The Rust port, by Björn
(`b-gehrke`) and contributors, uses MIT; its notice is preserved in
`LICENSE-WHELK`. See `NOTICE` for the repositories and pinned source revision.
Changes in this package are licensed under MIT OR Apache-2.0; see `LICENSE-MIT`
and `LICENSE-APACHE`. These project terms do not replace the upstream notices.
