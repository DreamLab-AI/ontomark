# ontomark-write

The headless write engine for ontomark working-memory pages: one validated,
lossless write path for vocabulary 3 vaults. The `ontomark` CLI's `edit`,
`create`, `apply` and `memory` commands write through it.

Every operation takes a snapshot of the vault, plans the write, validates the
result with the validator `ontomark validate` runs, and returns a `Plan`.
Nothing is written until `Plan::commit` applies it to a `Store`, and nothing is
written at all if the result would introduce a validation error.

| Operation | What it does |
|---|---|
| `Engine::create` | A new working page: minted `page_resource`, first `generated` stamp, visibility, initial status |
| `Engine::patch` | Changes only the named keys; every other byte is identical |
| `Engine::adopt` | Gives an existing page, such as a daily note, the identity it lacks |
| `Engine::transition` | Approve, reject, confirm, send back, withdraw or archive, per the review lifecycle |
| `Engine::record_disputes` | A draft names the approved pages it contradicts |
| `Engine::complete_supersession` | Finishes an approve-then-supersede that was interrupted |

## Guarantees and limits

- Per file, a write is atomic: a temporary file in the same directory is renamed
  into place.
- Across files, a plan is ordered, not crash-atomic. When one write fails, the
  earlier ones are restored in process. A kill between two writes leaves a
  half-done supersession, which `Engine::complete_supersession` finishes.
- `FsStore` is confined to the vault root and refuses symlinks.
- An approval or confirmation needs a human principal from the roster. An
  instrument's key is refused, whichever principal it acts for.
- Under vocabulary 2 only `patch` is available, and it passes through without
  the vocabulary 3 checks.

The crate documentation states each guarantee in full.

## Features

| Feature | Default | Purpose |
|---|---|---|
| `fs` | on | The atomic, root-confined file store. Off for wasm hosts, which apply a `Plan` themselves |
| `system` | on | Wall clock and OS randomness. Off for wasm hosts, which pass the time and entropy in |

## Licence

MIT OR Apache-2.0.
