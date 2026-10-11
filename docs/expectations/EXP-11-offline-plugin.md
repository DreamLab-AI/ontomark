---
id: EXP-11
linked_adrs: [ADR-2016]
priority: critical
regression_critical: true
evidence_category: partially-verifiable
status: accepted
authored_by: pair
---

## Expectation: the installed plugin performs every core operation with the network disabled

The release consists of `main.js`, `manifest.json` and `styles.css` only. The
wasm module and the worker source are embedded in `main.js` and instantiated
synchronously from bytes; no URL is fetched, no CDN, model, database or
service is contacted. Installed into `<vault>/.obsidian/plugins/ontomark/` it
indexes, classifies, queries and proposes and applies a guarded patch with no
network interface. Unload terminates the worker and removes every listener and
view, and a reload works.

### In scope
- Run inside a container started with `--network none`, plus an in-process
  tripwire that fails the run on any socket, DNS, fetch or XHR attempt.
- Static check of `main.js`: no `fetch(`, `XMLHttpRequest`, `import.meta.url`,
  `WebSocket` or `http(s)://` load paths in executable code.

### Limits (stated, not hidden)
- The real Obsidian/Electron renderer is not available in the build
  container; the Node harness substitutes a filesystem-backed `obsidian` API
  and a `worker_threads` Worker for the Blob-URL worker. A manual check in
  Obsidian desktop is listed in the plugin README.
- Mobile is out of scope for this sprint (`isDesktopOnly: true`).
