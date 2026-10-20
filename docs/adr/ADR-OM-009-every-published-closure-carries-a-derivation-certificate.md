# ADR-OM-009: every published closure carries a derivation certificate an independent checker re-verifies

- Status: Accepted (owner request of 20 October 2026)
- Implementation: active from v0.6.0. `crates/rdfs-certificate` (published to crates.io as `rdfs-certificate` 0.1.0: the RDFS engine, the `oo-cert/1` writer, the consequence delta), `crates/ontomark-compile/src/certificate.rs` (`spell`, `certify`, `parity`), `build/mod.rs` (`Options.certificate_out`, `CertificateParity`, `CertificateReport`, `Report.certificate`), `crates/ontomark-cli/src/main.rs` (`ontomark build --certificate-out`, exit 2 on parity loss). Tests: `rdfs-certificate` unit and `tests/oo_cert.rs` (runs the checker when `OO_CERT_BIN` is set), `certificate.rs` Whelk-parity test.
- Related: ADR-OM-005 (one producer), VisionClaw ADR-2147 (the same decision for the consumer side: `vault propose` carries the consequence delta; both now consume the one crate), agentbox ADR-2136.

## Context

`ontomark build` publishes `ontology-inferred.ttl`, the EL closure its classifier computes (81,396 subsumptions on visionGraph `1ec5774ad`). Loom serves it, VisionClaw lays it out, agents ground on it, and all of them take it on the classifier's word. Open Ontologies (fabio-rovai/open-ontologies v2.0.1, MIT) showed a cheaper warrant: a derivation certificate, two TSV files naming the rule and premises of every inferred triple, that a Lean 4 checker with a soundness theorem re-derives with no engine and no network. VisionClaw ADR-2147 (20 October 2026) internalised the engine for `vault propose` and measured, on the published corpus, that an RDFS certificate reproduces the classifier's named closure exactly, because this compiler emits restrictions only on the superclass side of `rdfs:subClassOf`. ADR-OM-005 makes ontomark the only producer of published bundles, so a warrant that only `vault` can write guards a path the corpus repository no longer builds with.

## Decisions

1. **The engine is one crate, consumed by both producers.** `rdfs-certificate` takes triples as three N-Triples-spelled terms and depends on no graph library, so it is the same code in ontomark (MIT OR Apache-2.0) and in VisionClaw (AGPL). The upstream format and rule table are kept so that the upstream checker re-checks what the crate writes; the upstream MIT notice travels in the crate as `LICENSE-UPSTREAM.txt`. The checker itself is never vendored, never a build dependency and never built from this repository: its independence from the engine is the warrant. CI and reviewers fetch it as a pinned release asset by tag and sha256.
2. **`ontomark build --certificate-out <dir>` certifies the graph it publishes**, from the very `Graph` that is reasoned and serialised, before any artefact is written. The report gains `certificate` (files, counts, `asserted.sha256`, timing, how to re-check). Without the flag nothing is written and nothing is checked, so the goldens and the bundle contract are byte-identical.
3. **Parity is enforced, not reported.** The certificate's named `rdfs:subClassOf` closure must equal the classifier's `inferred` set or the build is refused with `CertificateParity` (exit 2, the same refusal class as an unsatisfiable class): a generation is never published with a closure its warrant does not cover. Parity is structural today; the review trigger is the emitter placing a restriction or an equivalence on the subclass side, or a definition class whose EL consequences exceed subsumption transitivity. When that day comes the certificate table widens or the invariant narrows to the RDFS fragment, by a new record.
4. **The consequence delta ships in the crate but has no ontomark verb.** Proposals (`propose`, the kind-31402) stay in VisionClaw's `vault` under agentbox ADR-2136's deprecation window, and that is where the delta is shown to the signer. If `propose` moves into the governance adapter, the delta moves with it, by crate dependency and no port.

## Verification (2026-10-20, visionGraph `1ec5774ad+dirty`, checker `oo-cert` v2.0.1 sha256 `68223b4d…`)

- `ontomark build --vault all --certificate-out`: 25.9 s end to end; 351,421 asserted, 731,141 derivations (rdfs11 688,010; rdfs7 43,131); `certified_named_subsumptions` 81,396 = `classifier_inferred` 81,396; certify and write 4.5 s (the 305 MB `derivations.tsv` dominates).
- `oo-cert` on that directory: `{"ok":true,"asserted":351421,"derivations":731141,"theorem":"OOCert.certificate_sound"}` in 9.5 s.
- `cargo test --workspace` with `OO_CERT_BIN` set: all green, including the checker accepting a genuine certificate and refusing a forged line (`first_rejected: 0`), and `whelk_and_the_certificate_agree_on_the_named_closure`. clippy pedantic, rustdoc `-D warnings` and fmt clean. Goldens unchanged.
- The crate's delta was audited cross-vendor under ADR-2147 (one defect, derived-only comparison, fixed before this port and pinned by `asserting_what_was_already_entailed_is_conservative_and_dropping_an_assertion_is_not`).

## Alternatives considered

- A separate `ontomark certify` verb that rebuilds the graph. Rejected: it would reconstruct the projection a second time and could drift from what `build` published; certifying inside the build is the only way the warrant is for the bytes that shipped.
- Vendoring the Lean checker or writing one. Rejected: a checker that shares code or authorship with the engine warrants nothing.
- Keeping the engine inside each repository. Rejected on the owner's question of 20 October: the same code under two licences in two places is the shape that drifts; a permissive crate is consumed by both and attributed once.
