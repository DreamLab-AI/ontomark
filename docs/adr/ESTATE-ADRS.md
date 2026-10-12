# Estate ADRs that govern this workspace

These records live in their own repositories; this file points at them rather than copying them. Four copies kept here until 2026-10-12 had drifted from their sources (ADR-2014 61/85 lines, ADR-2016 51/78, ADR-VG-007 37/115, ADR-2136 41/69) and nothing checked them, so the copies were removed ([ADR-OM-005](ADR-OM-005-estate-alignment-one-producer-explicit-projections.md)). [`PLAN.md`](../PLAN.md) §ADRs carries the same links with one-line scopes.

| Repo | Record | Status (2026-10-12) | Governs |
|---|---|---|---|
| VisionFlow | [ADR-2014](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/adr/ADR-2014-shared-ontology-workspace.md) | accepted / partial / staged | shared workspace ownership; 2026-10-12 alignment section |
| VisionFlow | [ADR-2015](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/adr/ADR-2015-typed-ontology-model-and-migration.md) | proposed | typed model and reversible migration |
| VisionFlow | [ADR-2016](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/adr/ADR-2016-local-markdown-ontology-application.md) | accepted / partial | Obsidian application; ingest continuation |
| visionGraph | [ADR-VG-007](https://github.com/DreamLab-AI/visionGraph/blob/main/docs/adr/ADR-VG-007-shared-ontology-workspace-and-profile.md) | accepted / partial | corpus profile, CI producer, embedded explorer |
| VisionClaw | [ADR-2137](https://github.com/DreamLab-AI/VisionClaw/blob/main/docs/adr/ADR-2137-consume-shared-ontology-workspace.md) | proposed | native consumer adoption; 2026-10-12 decisions |
| VisionClaw | [ADR-2106](https://github.com/DreamLab-AI/VisionClaw/blob/main/docs/adr/ADR-2106-ontology-pull-model-into-the-embedded-pod.md) | accepted / live | pod pull channel (gate (iii) of VC-60 retirement) |
| Agentbox | [ADR-2136](https://github.com/DreamLab-AI/agentbox/blob/main/docs/adr/ADR-2136-pin-the-shared-ontology-toolchain.md) | proposed | toolchain repin; maps to ADR-2108 |
| Loom | [ADR-141](https://github.com/DreamLab-AI/loom/blob/main/docs/design/ADR-141-loom-consumes-the-vault-build.md) | proposed | generation identity `visionGraph@<sha>`; 2026-10-12 amendment |
| Loom | [ADR-142](https://github.com/DreamLab-AI/loom/blob/main/docs/design/ADR-142-consume-versioned-shared-ontology-contracts.md) | proposed | versioned shared contracts |
| vowl-wasm | [ADR-001](https://github.com/DreamLab-AI/vowl-wasm/blob/main/docs/adr/ADR-001-shared-ontology-rendering-contract.md) | implemented / active | shared rendering contract |

Ontomark's own records are the `ADR-OM-*` files beside this index.
