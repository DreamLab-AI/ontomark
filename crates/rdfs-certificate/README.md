# rdfs-certificate

RDFS forward chaining that records every derivation step in the `oo-cert/1`
certificate format, so an **independent** checker re-verifies a reasoner's
closure with no engine, no network and no trust in this crate. Also computes
the consequence delta between two graphs: what a change adds to, or removes
from, what a graph entails.

## Why

A reasoner's closure is a reasoner's word. A certificate is a warrant for it:
`asserted.tsv` lists the input, `derivations.tsv` lists every inferred triple
with the rule and the two premises that produced it, and a checker re-derives
each line from its premises by pattern alone. The format and rule table are
[Open Ontologies'](https://github.com/fabio-rovai/open-ontologies) (MIT), whose
Lean 4 checker `oo-cert` carries the soundness theorem `OOCert.certificate_sound`.
Forge a line and it exits 1 naming the rule.

## Use

```rust
use rdfs_certificate::{certify, consequence_delta, nt};

let sc = nt::iri("http://www.w3.org/2000/01/rdf-schema#subClassOf");
let base = [
    (nt::iri("urn:ex:espresso"), sc.clone(), nt::iri("urn:ex:coffee")),
    (nt::iri("urn:ex:coffee"), sc.clone(), nt::iri("urn:ex:drink")),
];
let cert = certify(base.iter().map(|(s, p, o)| (s.as_str(), p.as_str(), o.as_str())));
assert_eq!(cert.derivation_count(), 1); // espresso ⊑ drink, by rdfs11

let dir = tempfile::tempdir().unwrap();
cert.write(dir.path(), "my-graph@abc123").unwrap();
// oo-cert <dir>/asserted.tsv <dir>/derivations.tsv  →  {"ok":true, ...}

let mut proposed = base.to_vec();
proposed.push((nt::iri("urn:ex:drink"), sc.clone(), nt::iri("urn:ex:liquid")));
let after = certify(proposed.iter().map(|(s, p, o)| (s.as_str(), p.as_str(), o.as_str())));
let delta = consequence_delta(&cert, &after);
assert!(!delta.is_conservative());
assert_eq!(delta.new_consequences, 3); // the asserted line and two inherited parents
```

Input is three N-Triples-spelled terms per triple, so the crate depends on no
graph library; `nt` spells IRIs, blank nodes and literals (with escaping).

## Rules

rdfs2, rdfs3, rdfs5, rdfs7, rdfs9 and rdfs11, premises in the checker's order.
A literal never becomes a subject. Terms are compared by spelling, never by
value. On a 351k-triple, 9.5k-class corpus: 731k derivations in 1.2 s; the
checker accepts in 9 s.

## Licence

MIT OR Apache-2.0. The certificate format and rule table are adapted from
Open Ontologies v2.0.1, © 2026 Fabio Rovai, MIT; its notice is kept verbatim
in `LICENSE-UPSTREAM.txt`.
