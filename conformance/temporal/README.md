# Estate temporal contract (`estate-temporal-v2`)

The single meaning of a time-qualified ontology fact across the estate
(VisionFlow ADR-2022 P1; VisionClaw ADR-2219). VisionClaw's provenance writer
is the reference implementation of v1 and is being moved to v2; the executable
reference for v2 is the test described under "Executable check". Any other consumer that records or queries
history, including ontomark's typed model (ADR-2015), uses these terms and this
rule rather than its own.

The vocabulary for the two clocks and for closing them is adopted from
[memory-ontology](https://github.com/eljeffeg/memory-ontology) by eljeffeg,
licensed [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Its `memory:ValidTime` and `memory:RecordedTime` are used
unchanged (IRIs as at its `origin/main`, commit `8ecc9ed`); the closure record itself is the estate's own, in the `dl:` namespace.

## Model

Two clocks, never merged:

- **Valid time**: when a fact holds in the world.
- **Recorded time**: when the system held the record.

A **version** states one fact (subject, predicate, object) from `validFrom`,
recorded at `generatedAt`. A version never carries an end and is never edited.

A **closure** is a separate record that ends a version on one or both clocks.
A closure on valid time is a world change ("it stopped being true at X"). A
closure on recorded time is a correction ("the record was wrong"). Closures are
themselves never closed. A wrong correction is undone by asserting a new
version, which must carry a later `generatedAt` and so has a different identity.

## Terms

Namespace `dl:` is `https://narrativegoldmine.com/ns/dl#`; `prov:` is
`http://www.w3.org/ns/prov#`; `memory:` is `http://nova.dev/memory#` and
`temporal:` is `http://nova.dev/temporal#`. The two `temporal:` properties and
the two `memory:` clock concepts are used by IRI from
[eljeffeg/memory-ontology](https://github.com/eljeffeg/memory-ontology)
(CC BY 4.0).

| Term | IRI | Meaning |
|---|---|---|
| Version | `prov:Entity` with `rdf:subject`, `rdf:predicate`, `rdf:object` | One fact, as asserted. |
| Valid-time start | `temporal:validFrom` (`http://nova.dev/temporal#validFrom`) | When the fact became true. Inclusive. On the version. |
| Recorded time, version | `temporal:recordedFrom` (`http://nova.dev/temporal#recordedFrom`) | When the version was recorded. A version carries no `prov:generatedAtTime`. |
| Recorded time, closure | `prov:generatedAtTime` | When the closure was recorded. |
| Closure | `dl:Closure` | A record that ends a version on at least one clock. Not a version: no subject, predicate or object. |
| Closed version | `dl:closesVersion` | The version a closure ends. |
| Clock closed | `dl:closes` | `memory:ValidTime` and/or `memory:RecordedTime` (`http://nova.dev/memory#ValidTime`, `http://nova.dev/memory#RecordedTime`). One or both, never neither. |
| Valid-time end | `dl:validUntil` | When it stopped being true. Exclusive. Present if and only if the closure closes valid time. |
| Generating activity | `prov:wasGeneratedBy` | The activity that recorded the version or closure. |
| Agent | `prov:wasAttributedTo` | Who is responsible. Every version and closure has one (`agent` in the JSON). |
| Reason | `dl:reason` | Optional free text for application policy. No effect on state. |
| Replacement | none | `replacedBy` in the vector file only: names the version that supersedes a closed one. Informative; no effect on state. |

`dl:validFrom` and `dl:validTo` are retired from versions and appear in no v2
data. memory-ontology's `temporal:validUntil` and `temporal:recordedUntil` are
not used: they put an end on the earlier record, and an end with no recorded
time of its own is the v1 defect this contract removes.

The property IRIs are used without a claim of conformance to memory-ontology's
shapes. Its records reify with RDF 1.2 `rdf:reifies`; a version here reifies
with `rdf:subject`, `rdf:predicate` and `rdf:object`, and a closure is not part
of its model.

In the vector file, v1's three versions keep the v1 form: an inline `validTo`
is shorthand for a valid-time closure recorded at that version's `generatedAt`,
and a harness realises it as a closure record. Every other end in the file is
an explicit closure. The three temporal literals (`temporal:validFrom`,
`temporal:recordedFrom`, `dl:validUntil`) are `xsd:dateTimeStamp`; a closure's
`prov:generatedAtTime` is `xsd:dateTime`. All are canonical UTC, second
precision, `Z` suffix. The JSON keys (`validFrom`, `generatedAt`, `validTo`,
`recordedAt`) are names in the vector file, not RDF terms.

In the JSON, `closes` lists `"valid"` and/or `"recorded"`, standing for the two
`memory:` clocks.

## The state rule

Let `t` be a valid-time instant and `r` an optional recorded-time cut
(`recordedAsOf`). A version `v` is in `state(t, r)` if and only if all of:

1. `v.generatedAt <= r`, or `r` is absent;
2. `v.validFrom <= t`;
3. no closure of `v` that closes recorded time has `recordedAt <= r`;
4. no closure of `v` that closes valid time has `recordedAt <= r` and
   `validUntil <= t`.

When `r` is absent, every closure applies (`recordedAt <= r` is taken as true).
Several valid-time closures on one version each exclude independently, so the
effective end is the earliest `validUntil` among those that apply; a later
closure cannot extend an interval. The `validTo` reported in an API row is that
effective end.

Consequences the vectors exercise: a close recorded after the cut is invisible
to it; a recorded-time close removes the version at every valid instant; a
closure on both clocks applies each part by the same clause as a closure on one;
both boundaries are inclusive for recorded time (`<=`) and the valid end is
exclusive (`validUntil <= t` excludes).

## Writer rules

1. **Half-open intervals.** A version is valid from `validFrom` inclusive to the
   effective end exclusive. Ending one version and starting its successor at the
   same instant leaves exactly one current, never zero or two.
2. **Append only in operation.** Ending a version adds a closure. There is no
   runtime path that edits or deletes a version or a closure, and a closure is
   never closed. Expiry and retraction are closures, not a second mechanism.
   The two named exceptions are an operator-approved refit of the contract
   itself, recorded in the decision record and not in the data, and erasure
   (VisionClaw ADR-2102).
3. **Two clocks, never merged.** As-of queries filter on valid time, and
   optionally on recorded time for a knowledge cut. Replication and sync order
   use recorded time (or their own sequence), never valid time. An as-of answer
   can change later, when a late edit or a backdated close is recorded.
4. **Reasoning sees one instant.** A reasoned view at `t` is built by
   projecting `state(t, r)` and saturating that projection. A reasoner is never
   run over the union of versions.

Version identity includes recorded time: it is derived from subject, predicate,
object, `validFrom`, `generatedAt` and agent, so a re-assertion or a
reinstating version never takes the identity of an earlier one. A closure's
identity is derived from the version, the clocks, `validUntil` and `recordedAt`, under the
agent.

## Out of the contract

- Federation tombstones: cache eviction of received copies, not a temporal fact.
- Erasure (VisionClaw ADR-2102).
- A page left `public: true` but `status: deprecated` receives no closure.
- Where a correction's replacement starts. The rule does not decide whether the
  replacement's `validFrom` is backdated to the original's (as `v7` is) or set
  to the correction time; that is a data choice for the writer.
- Query syntax. SPARQL, SQL or code are consumers' choice; the rule above is
  normative and the vectors are the test.

## Vectors

[`vectors-v2.json`](vectors-v2.json) holds versions, closures and expected
`state(t, recordedAsOf)` results. A conforming implementation returns exactly
the expected version ids for every case. Its first three versions and nine cases
are those of v1 and keep their answers. Further cases cover a backdated close
queried either side of the moment it was recorded; a correction with and
without a replacement; a closure on both clocks; two valid-time closures on one
version; a wrongly corrected fact reinstated by a new version with the same
subject, predicate, object and `validFrom` but a later `generatedAt`; an absent
`recordedAsOf`; and the boundary instants.

Consumers vendor the file and pin its SHA-256:

```
afd2bca7acfadbe455abf5f834e5069133c6f8f7b6229044031be42881b8c134  vectors-v2.json
```

Changing the file is a contract change: bump `contract`, record why, and update
every consumer's pin in the same release.

### Executable check

`cargo test -p ontomark-reason --test temporal_contract` (run by the `check` job in
`.github/workflows/ci.yml` through `cargo test --workspace`) loads both vector
files, verifies both pins and that this README quotes them, validates the closure
records, and asserts every case against a short reference evaluator of the rule
above. A negative test shows the check can fail: the v1 rule, and a rule with a
strict `<` on recorded time, are each rejected by the v2 vectors. The test is
in `ontomark-reason`'s test directory because that crate already has both
`serde_json` and `sha2` as dev-dependencies, so it adds no dependency and edits
no manifest. Closure records are also validated: at least one clock, `validUntil`
iff valid time is closed and after `validFrom`, recorded no earlier than the
version, an agent named, and no subject, predicate or object.

| Consumer | Check |
|---|---|
| ontomark | `cargo test -p ontomark-reason --test temporal_contract` |
| VisionClaw | v2 adoption pending (VisionClaw ADR-2219): its vendored copy and pin move to `vectors-v2.json` |

## v1

`estate-temporal-v1` ([`vectors.json`](vectors.json)) is superseded. It knows
only an inline `validTo` and no closure records, and so cannot express a
backdated close, a correction or a reinstatement. It is kept, byte for byte,
until its last consumer moves to v2, because VisionClaw pins its hash:

```
6973aeef35e538412375dd6e4a9d9539f12f88fa7513d237371098da3bc46f75  vectors.json
```

Do not edit it. Delete it, with this section, once no consumer pins it.
