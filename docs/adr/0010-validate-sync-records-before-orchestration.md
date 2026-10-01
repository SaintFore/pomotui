# Validate synchronization records before orchestration

Synchronization records use kind-specific sum types with validated global identities and mutation time rather than a loose envelope containing optional fields. A protocol-neutral Sync Engine owns document validation, set union, and deterministic projection planning, while the Timer Service owns durable application and scheduling; this keeps invalid states outside product logic and lets background file I/O evolve without moving domain ownership into the filesystem adapter.

## Consequences

The pre-release sync format may change incompatibly while ADR 0009 permits full local-data reset. New synchronized entity kinds must extend the validated record model and projection rules instead of adding string switches and optional payload fields to Timer Service command handling.

File operations run on one single-flight synchronization worker rather than under the Timer Service state lock. Startup, relevant local mutations, and a fixed 30-second interval coalesce work onto that worker; file failure updates synchronization health and schedules later retry without degrading local durable health or undoing an already durable mutation. Before replacement, the worker compares the source fingerprint and re-reads a changed document a bounded number of times; any final narrow race is repaired by a later union with records retained in local databases.
