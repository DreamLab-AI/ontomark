<div align="center">

# ontomark

### Ontologies in Markdown

[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue?style=flat-square)](LICENSE-MIT)
[![Targets](https://img.shields.io/badge/Targets-native%20%2B%20wasm32-8b5cf6?style=flat-square)](#crates)
[![Reasoning](https://img.shields.io/badge/Reasoning-OWL%202%20EL%20%28Whelk%29-10b981?style=flat-square)](crates/whelk-el)
[![Status](https://img.shields.io/badge/Status-sprint%20in%20progress-f59e0b?style=flat-square)](docs/RUNLOG.md)

**Maintainer**: [John O'Hare](https://github.com/jjohare) · **Owner**: [DreamLab-AI](https://github.com/DreamLab-AI) · **Reasoner lineage**: [Whelk](https://github.com/balhoff/whelk) by Jim Balhoff ([NOTICE](NOTICE))

[What it is](#what-it-is) · [Crates](#crates) · [Standards](#standards-first-superset-as-a-profile) · [Quickstart](#quickstart) · [Where it sits](#where-it-sits-in-the-estate) · [Design record](#design-record) · [Status](#status)

</div>

---

> **A folder of Markdown notes is an ontology if something can read it as one.** ontomark is that something: a typed model, an OWL 2 EL reasoner, a compiler and a graph codec that treat Obsidian-style Markdown as the authoritative source of a formal ontology, run natively and in the browser, and never require a database. It replaces four parsers, two reasoners and four graph codecs that grew up separately across the DreamLab estate, and it is built so that anyone with a vault can use it without knowing the estate exists.

---

## What it is

ontomark reads a vault of Markdown pages whose YAML frontmatter declares classes, individuals and properties, resolves every reference to an IRI, checks the result against the OWL 2 EL profile, classifies it with a Whelk reasoner, and compiles it to standard serialisations and to a compact binary graph for rendering. Writes go back through a guarded patch API that changes only the keys it was asked to and proves the rest of the file is byte-identical.

Three things it refuses to do: hold truth anywhere but the Markdown, infer structure with a language model, or ship an estate rule as a hard-coded constant. Everything estate-specific lives in a versioned profile.

## Crates

| Crate | Owns | Targets |
|:------|:-----|:--------|
| `ontomark-parse` | Lossless Markdown document model: frontmatter with order, comments and quoting preserved; body spans; wikilinks and footnotes; guarded patches | native, wasm32 |
| `ontomark-model` | Typed ontology model: `Class`, `Individual`, `Property`; full predicate IRIs via a prefix map; reference states `Resolved`, `Ambiguous`, `Unresolved`, `External`; the `Profile` trait | native, wasm32 |
| `whelk-el` | OWL 2 EL reasoner, a fork of whelk-rs with the CLI and all host dependencies removed; `rayon` optional | native, wasm32 |
| `ontomark-reason` | EL-profile checker with precise refusal reasons; classification facade; cancellable incremental generations for worker hosts | native, wasm32 |
| `ontomark-compile` | RDF 1.1 Turtle, JSON-LD 1.1, SHACL shapes, PROV-O attribution and public projections; re-exports the shared NGG1 codec | native, wasm32 |
| `ontomark-ngg1` | NGG1 graph tier encoder and zero-copy reader; published as 0.1.0 | native, wasm32 |
| `ontomark-wasm` | `wasm-bindgen` surface for Obsidian and the browser: index, update, classify, query, patch | wasm32 |
| `ontomark-cli` | `ontomark build / validate / gate / conflicts / migrate / attribute / assets / body-residue`; remaining estate commands are pending | native |

`plugin/` is the Obsidian plugin: self-contained, offline, no database, a local SPARQL subset, guarded writes behind a diff preview. Desktop first.

## Standards first, superset as a profile

ontomark is a strict consumer and a correct producer of the open stack, then layers its own rules as a named, versioned profile that any plain OWL tool can ignore.

| Layer | Standard |
|:------|:---------|
| Reasoning profile | OWL 2 EL |
| Serialisations | RDF 1.1 Turtle, JSON-LD 1.1 |
| Labels, aliases, definitions | SKOS |
| Attribution | PROV-O (`prov:wasAttributedTo`) |
| Validation rules | SHACL |
| Metadata | Dublin Core Terms |

The **generic profile** accepts a vault of one note. The **estate profile** (`profiles/estate`) adds the DreamLab frontmatter grammar, lifecycle and governance vocabulary, `urn:ngm:class:` and `resource:` IRI rules, a 53-category taxonomy held in one table, and an attribution roster that resolves `generated.by` actors to `did:nostr` identities. Signing is a trait; `did:nostr` is one implementation and the default surface is unsigned.

## Quickstart

```bash
source scripts/env.sh            # repo-local cargo home
cargo build --workspace
cargo test --workspace
cargo build -p ontomark-wasm --target wasm32-unknown-unknown
```

`just` targets include `just check`, `just wasm`, `just wasm-test`, `just conformance`, `just doc` and `just plugin`. CI runs clippy with warnings denied, `missing_docs` denied workspace-wide, the wasm32 build, and gitleaks.

## Where it sits in the estate

ontomark is the shared ontology core named in VisionFlow ADR-2014. It is consumed, not depended on, by the mesh:

| Consumer | Takes from ontomark | Keeps |
|:---------|:--------------------|:------|
| **[visionGraph](https://github.com/jjohare/visionGraph)** | the CLI in `publish.yml`, the NGG1 codec in its explorer | the corpus, the estate profile's data |
| **[VisionClaw](https://github.com/DreamLab-AI/VisionClaw)** | model, parse, reason, compile crates | a thin AGPL `vault` adapter owning `propose`, `apply` and panel publishing |
| **[vowl-wasm](https://github.com/DreamLab-AI/vowl-wasm)** | NGG1 via re-export | its published crate and JS API |
| **[loom](https://github.com/DreamLab-AI/loom)** | versioned bundle contracts | its MIT boundary and model-swap façade |
| **[agentbox](https://github.com/DreamLab-AI/agentbox)** | the baked CLI by release tag | the AGPL adapter alongside it |
| **[knowledgeGraph](https://github.com/DreamLab-AI/knowledgeGraph)** | the published artefacts | nothing of its own; source of truth is visionGraph |

Every duplicate the census found is listed with its replacement and its removal gate in [`docs/DEPRECATION.md`](docs/DEPRECATION.md).

## Design record

- [`docs/adr/ADR-OM-001-sprint-decisions.md`](docs/adr/ADR-OM-001-sprint-decisions.md): the decisions this repository was built under, and the facts the census established.
- [`docs/adr/`](docs/adr/): the eight estate ADRs (VisionFlow 2014, 2015, 2016; visionGraph VG-007; VisionClaw 2137; Agentbox 2136; Loom 142; vowl-wasm 001) that proposed it.
- [`docs/PLAN.md`](docs/PLAN.md): work packages WP-01 to WP-08 and the twelve permanent anti-drift tests EXP-01 to EXP-12.
- [`docs/SPRINT-BRIEF.md`](docs/SPRINT-BRIEF.md) and [`docs/RUNLOG.md`](docs/RUNLOG.md): how the first sprint is organised and what has happened so far.

## Status

The five core work packages are integrated locally. The combined native gate passes 746 tests (7 ignored); reasoner WASM tests, plugin checks and legacy corpus parity also pass. `ontomark-ngg1` 0.1.0 is published on crates.io. The CLI release and estate migrations remain in progress; see [the hand-off](docs/HANDOFF.md) and [branch census](docs/BRANCH-RECOVERY.md).

The reasoner implements a subset of OWL 2 EL. Capability metadata reports that limit; classification refuses unsupported constructs and reports dropped existential restrictions. The compiler still uses its legacy classification path while its move to the shared reasoner is pending. The offline plugin checks use an Obsidian runtime shim; an installed desktop Obsidian acceptance run remains outstanding.

## Licence

`MIT OR Apache-2.0` for every crate here. Code ported from DreamLab's AGPL repositories is relicensed by its sole rights holder; the grant and its exclusions are in [`NOTICE`](NOTICE). The Whelk reasoner lineage is MIT and its notice travels with `whelk-el`.
