# Changelog

All notable changes to `@dreamlab-ai/ontomark-graph`. Releases are GitHub release assets (tag `ontomark-graph-vX.Y.Z`, asset `dreamlab-ai-ontomark-graph-X.Y.Z.tgz` with its `.sha256`), not npm.

## 0.4.0 — 2026-11-23

Added, all opt-in; without them the layout is exactly the 0.3.0 one:

- `createLayout` option `params` and live `layout.setParams(params)`, which pass engine parameters to an engine with `setParams`, merge over earlier values and restart a loaded layout warm. Both return or report keys the engine cannot apply (`{applied, ignored}`, option `onIgnoredParams`).
- `vowlEngine` engines gain `setParams` and `supportedParams`. Keys are applied through `NggExplorer.setParam` only where the bundle's `version()` has them, so an older vowl-wasm keeps working with its defaults; applied values are set again after every load. `reheat` is never applied as a parameter.
- `RECOMMENDED_LAYOUT_PARAMS`: `collideRadius` 21, `collideStrength` 0.7, `collideIterations` 3, `componentGravity` 0.1, the vowl-wasm 0.4.0 collision and component-gravity settings for nodes drawn at up to 12 px radius plus a label.

Tests now run the driver against `@dreamlab-ai/vowl-wasm` 0.4.0, a dev dependency pinned with its integrity, in CI as well.

## 0.3.0 — 2026-11-14

Host-side layout driver at `@dreamlab-ai/ontomark-graph/layout` (`createLayout`, `layoutInput`, `vowlEngine`); renderer `ariaLabel`, `preservePositions` on `setGraph`, and an explicit node `shape`.

## 0.2.0 — 2026-10-30

Node dragging in the shared renderer (`onDragStart`, `onDrag`, `onDragEnd`).

## 0.1.0

Shared offline graph renderer, traversal and hierarchy adapter.
