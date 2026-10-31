---
id: ADR-OM-008
title: "any Obsidian vault is a repository; folders and generation prefixes come from the profile"
date: 2026-10-17
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: "3cb298c9dd44084756b1227b954d2716347cd296"
verified_paths: []
owner: jjohare
review_trigger: "a change to resolve_repo, to the profile Layout, or to the generation naming contract the estate consumes"
repo: ontomark
---

# ADR-OM-008: any Obsidian vault is a repository; folders and generation prefixes come from the profile

- Status: Accepted (owner request of 17 October 2026)
- Implementation: active from v0.4.0. `crates/ontomark-cli/src/main.rs` (`resolve_repo`, `resolve_profile`, `vault_root`), `crates/ontomark-compile/src/profile.rs` (`Layout::{knowledge,working,dir_of,path_of,root_of,kind_of,flat}`, `MissingVault`, `GenerationNaming`), `input/page.rs` (`VaultKind::role_name`), `build/generation.rs` (`generation_id(prefix, commit, dirty)`), `input/vocabulary.rs` (`Vocabulary::generic`), `check/validate.rs` (role grammar from the profile). Tests: `crates/ontomark-cli/tests/plain_vault_cli.rs`, `profile.rs` unit tests, `tests/bundle_contract.rs`.
- Related: ADR-OM-007 (the vault root is the content root), ADR-OM-005 (one producer), ADR-OM-001 (profiles as data).

Status mapping 2026-10-31: the record has no date line; 2026-10-17 is taken from its status text ("owner request of 17 October 2026"). the record's `Implementation: active from v0.4.0` is not a checker value. `activation_status: live` follows its word "active"; `implementation_status: partial` is the conservative reading because the record does not say "complete". Owner to confirm.

Verification 2026-10-31: `verified_commit` is this record's own last-modified commit, recorded when the record was moved onto the estate ledger format. It is not a re-verification of the code the record describes, and `verified_paths` is empty, so the staleness gate is not yet armed for this record.

## Context

ADR-OM-007 made each vault an ordinary Obsidian vault, but the tooling around it still assumed the DreamLab estate's repository: `ontomark` refused to start without `ontology/vocabulary.yaml` in an ancestor, so `--profile generic` was unreachable for anyone without that shape; the two vault roles were located by the literal folder names `knowledge/` and `working/` in sixteen places, and a vault was deemed "working" because its directory was called `working`; every generation id was spelled `visionGraph@<sha>` by a constant; and the validator required `type` and `status` on every knowledge page and refused undeclared keys whatever the profile said. An audit on 17 October 2026 listed these as the gaps between "flat vaults" and "any vault".

The estate's contract must not move: its generation ids, both golden suites and the output tree (`publish/pages/`, `publish/working/`, reports keyed `knowledge`/`working`) are what Loom, VisionClaw and agentbox consume.

## Decisions

1. **A directory without `ontology/vocabulary.yaml` above it is a plain Obsidian vault and is the repository root.** `resolve_repo` still walks up looking for the vocabulary file; when none exists, `--repo` (or the current directory) is the root, the profile defaults to generic, and the vocabulary is `Vocabulary::generic()` — schema version 1, the built-in prefixes, the `urn:ontomark:` namespaces, nothing declared. A vocabulary file that exists but does not parse is still an error. A built-in profile on a plain vault takes `Layout::flat()`; a profile YAML keeps the layout it declares. Where the file is found, nothing changes: the estate behaviour is byte-identical, proven by `conformance/parity.sh` and the `estate-small` golden.
2. **Vault folders are layout data, not constants.** `profile::Layout` gains `knowledge: String` (default `knowledge`) and `working: Option<String>` (default `Some("working")`) beside `excluded`. `"."` is a legal `knowledge` value meaning the repository root is the vault; `working` may not be `"."` or equal to `knowledge`. `Layout::dir_of/path_of/root_of` place a role on disk and `Layout::kind_of` reads a root's role back; every site that used to join a folder name now asks the layout (the CLI's `load`, `build::Options.vault_root`, `publish::sibling_vault`, the machine-scope working root, `migrate` plan and compile, `repair` scopes). `VaultKind::dir_name` is renamed `role_name` and kept for the places where the name is an **output key** — the staging `per_vault` map, the validation/gate/conflicts report keys, the digest paths, the plan's `vaults` — so the published contract does not move. A role the layout does not place (`working: null`, or its folder absent) is refused by name with `MissingVault`: `--vault working|all` and `--scope machine` exit 2 with a message that quotes the layout, never a panic or a bare "not a directory".
3. **The generation prefix is the profile's.** `generation_id(prefix, commit, dirty)` replaces the constant; `Profile.generation: GenerationNaming { prefix: Option<String> }` supplies it, defaulting to the repository root's directory name (after resolving `.` and symlinks; `ontomark` for a nameless root). `profiles/estate/profile.yaml` pins `generation: {prefix: visionGraph}`, so the estate's ids and both goldens are unchanged. The bundle reader (`BundleManifest::validate`) checks the prefix **from the id itself** — everything before the last `@`, non-empty — so a consumer needs no profile to validate a marker it is handed.
4. **The validator reads the role grammar from the profile.** `UNKNOWN_KEY` fires when the role's `unknown_keys` is `fail`; `MISSING_TYPE` and `MISSING_STATUS` when the knowledge role lists `type`/`status` among `required_keys`. The estate profile declares exactly the former hard-coded rule, so its reports are unchanged; the generic profile's permissive knowledge role lets a plain vault validate. Required keys are enforced on the knowledge role only; the working role stays tolerant (OKF §4.1), as before.

## Alternatives considered

- Making `Profile::generic()`'s layout `"."` by default. Rejected: `Layout::default()` is also the serde default for a profile file that omits `layout:`, which the estate profile does; the estate would then have had to pin its folders. The CLI applies `Layout::flat()` only when no vocabulary file is found and the profile is a built-in.
- Taking the manifest prefix from the profile when validating a bundle. Rejected: readers (Loom) validate markers without a profile; the id already carries its prefix.
- Keeping `dir_name()` as an alias. Rejected: a method named for a directory would keep inviting folder assumptions. The rename is a breaking change and the crate goes to 0.4.0.

## Consequences

- `ontomark-compile` 0.4.0 and workspace 0.4.0 (CLI and engine ship in one stream): `generation_id` and `generation()` take a prefix, `Layout` has two new fields, `dir_name` is `role_name`, `load_vault`/`vault_kind_of` are gone, `sibling_vault` and `repair::scopes_for` take layout-resolved roots. The wasm transcript golden is re-blessed for the version strings only.
- The CLI is published to crates.io as `ontomark` (`cargo install ontomark`), which requires `ontomark-profile-estate` to be published first with full metadata; the estate profile remains a dependency only because `--profile estate` is built in.
- `ontomark --repo <any vault>` validates, builds, queries and migrates a plain Obsidian vault with the generic profile; the generation id carries the vault's directory name.
