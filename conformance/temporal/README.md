# Estate temporal contract (`estate-temporal-v1`)

The single meaning of a time-qualified ontology fact across the estate
(VisionFlow ADR-2022 P1). VisionClaw's provenance writer is the reference
implementation (`src/services/provenance_writer.rs`, legacy ADR-049); any other
consumer that records or queries history, including ontomark's typed model
(ADR-2015) and federation leases and tombstones (VisionClaw VC-48), uses these
terms and these rules rather than its own.

## Terms

| Axis | RDF term | Meaning |
|---|---|---|
| Valid time, start | `dl:validFrom` (`https://narrativegoldmine.com/ns/dl#validFrom`) | When the fact became true. Inclusive. |
| Valid time, end | `dl:validTo` (`https://narrativegoldmine.com/ns/dl#validTo`) | When it stopped being true. Exclusive. Absent means still true. |
| Recorded time | `prov:generatedAtTime` | When the system learned of it. |

All three are `xsd:dateTime` in canonical UTC, second precision, `Z` suffix.

## Rules

1. **Half-open intervals.** A version is valid at `t` iff
   `validFrom <= t` and (`validTo` is absent or `t < validTo`). Ending one
   version and starting its successor at the same instant leaves exactly one
   current, never zero or two.
2. **Append only.** Retraction adds `dl:validTo` to the open version; nothing
   is deleted or rewritten. Expiry and retraction (including federation
   tombstones) are a `dl:validTo`, not a second mechanism.
3. **Two clocks, never merged.** As-of queries filter on valid time, and
   optionally on `generatedAt <= recordedAsOf` for a knowledge cut. Replication
   and sync order use recorded time (or their own sequence), never valid time.
   An as-of answer can improve later, when a late edit is recorded.
4. **Reasoning sees one instant.** A reasoned view at `t` is built by
   projecting the versions valid at `t` and saturating that projection. A
   reasoner is never run over the union of versions.

## Vectors

[`vectors.json`](vectors.json) holds versions and expected `state(t, recordedAsOf)`
results. A conforming implementation returns exactly the expected version ids
for every case. Consumers vendor the file and pin its SHA-256:

```
6973aeef35e538412375dd6e4a9d9539f12f88fa7513d237371098da3bc46f75  vectors.json
```

Changing the file is a contract change: bump `contract`, record why, and
update every consumer's pin in the same release.

| Consumer | Check |
|---|---|
| VisionClaw | `cargo test --lib temporal_contract`: runs the pure `state_at` and the `state-at` SPARQL over an in-memory store against the vendored copy |
