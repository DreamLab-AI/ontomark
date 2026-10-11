---
id: EXP-06
linked_adrs: [ADR-2016]
priority: critical
regression_critical: true
evidence_category: executable
status: accepted
authored_by: pair
---

## Expectation: a worker result is applied only if it belongs to the current source

The host owns a monotonic generation counter and a worker epoch. Every vault
mutation (create, modify, delete, rename, profile change) bumps the generation
before it is sent. Every request and response carries `{epoch, generation, id}`.
The host applies a classify or query result only when its epoch equals the
live worker's epoch, its generation equals the current generation, and its id
is the latest request of that kind. A classify request that the worker dequeues
after a newer mutation is answered `stale` without running. Cancel-on-edit
either interrupts a running classify between reasoner steps or terminates the
worker; a respawned worker gets a new epoch and is replayed from the vault.

### In scope
- Edit during classify: old result is discarded and a new classify is scheduled.
- Delete during classify: the deleted page's entities never reappear.
- Kill and respawn during classify, then inject a late old-epoch result: rejected.
- Out-of-order responses (newer arrives first): the older one is rejected.

### Counter-examples (must NOT happen)
- A result computed for generation N displayed after generation N+1 was sent.
- A respawned worker's generation numbers colliding with a dead epoch's.
