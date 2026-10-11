---
id: EXP-12
linked_adrs: [ADR-2016]
priority: high
regression_critical: true
evidence_category: executable
status: proven
evidence: docs/evidence/WP-E/README.md
stabilized_by: "crates/ontomark-wasm/src/query.rs::tests; crates/ontomark-wasm/tests/transcript.rs; plugin/test/e2e/offline.e2e.cjs"
authored_by: pair
---

## Expectation: local SPARQL is read-only, bounded and derived entirely from Markdown

Queries run over an in-memory graph compiled from the vault's frontmatter; the
graph has no persistence and rebuilding it from the same files gives the same
results. Supported: SELECT, ASK and CONSTRUCT with basic graph patterns, joins,
FILTER on literals, OPTIONAL, UNION, ORDER BY, LIMIT/OFFSET, DISTINCT, and the
dataset selection `asserted`, `inferred` or `combined` (also exposed as named
graphs). Rejected with an explicit diagnostic: SPARQL UPDATE, SERVICE
(anywhere in the algebra, including inside OPTIONAL, subqueries and FILTER
EXISTS), FROM/FROM NAMED, DESCRIBE and extension functions. Results are capped by a
row limit and the evaluation is cancelled when the generation moves on.

### Counter-examples (must NOT happen)
- Any query changing a file or the index.
- A SERVICE clause reaching the network or being silently ignored.
- Different results from native and wasm for the same fixture and query.
