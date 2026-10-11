# Typed compiler API migration

Compiler 0.1.1 publishes the strict C3 marker and shared scaffold/prose reader
types. Compiler 0.1.2 adds the optional `model-projection` feature and moves the
native class adapter onto the shared supported-profile reasoner.

Readers that only need `bundle` should keep `default-features = false`. In-memory
consumers add `features = ["model-projection"]`, build a normative `Model` through
their injected model profile, then call `projection::project_model` with an
explicit authoring or public scope. Returned graphs and logical axioms contain
the same selected entity view; there is no filesystem or publication side effect.

The typed route intentionally changes these rc1 conventions:

| Source case | Typed route | Legacy native route |
| --- | --- | --- |
| Declared class with only SKOS relations | Always declares `owl:Class`; SKOS remains non-logical | Existing rc1 output retained |
| Unresolved parent or missing identity | Diagnostic/omission; no invented class | Existing corpus projection conventions retained |
| Several documents claim one IRI | All claimants refused; no first claimant | Existing corpus refusal/compatibility rules retained |
| Private target or alias in public scope | Withheld before RDF and classification; literals naming it withheld whole | Existing shared public redaction retained |
| Individual class assertion | RDF type preserved; shared classifier refusal makes result incomplete | Legacy class-only extraction retained |
| Data assertion | Typed literal preserved; reasoning omission makes result incomplete | Legacy annotation path retained |

ADR-OM-001 records 690 legacy classes typed only as SKOS concepts. The typed API's
class-declaration test establishes the intended rule for those declarations;
this release does not claim a full estate rebuild through a normative estate
profile, or change that count in the legacy bundle. Native CLI golden parity
remains a separate gate. `--no-redact-private-links` continues to select the
recorded legacy comparison mode; it does not enable phantom identities in the
typed API.

`ProjectionClassification::is_complete` includes model omissions and shared
reasoner refusals. `is_consistent` also requires no unsatisfiable named classes.
Inspect the shared refusal records and identifier-free omission counters to
distinguish unsupported reasoning from a logical clash. The shared classifier's
own completeness applies only to the logical axioms it received.

Native `reason` refuses a classifier's unsupported input before saturation or
bundle output. Forced-mode `reason_with` retains refusal diagnostics and returns
no saturation results for refused input. Its consistency verdict is false.

Synthetic tests cover the declaration, identity, privacy, restriction, individual,
data and budget cases. No private estate page bodies are needed in these tests.
