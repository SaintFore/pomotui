# 02: Converge the Task lifecycle

**What to build:** Make the complete Task lifecycle converge automatically through the sync file, including independent offline edits and deletion, while preserving the local Current Session and existing Task invariants.

**Blocked by:** 01 / Sync one Task through one file.

**Status:** resolved

- [x] Task creation, rename, completion, reopening, and deletion are represented as append-only, globally identified synchronization records.
- [x] Devices that observe the same records derive the same Task state regardless of import order.
- [x] Concurrent mutable versions resolve by recorded mutation time and stable identity tie-breaker, and a later valid edit can supersede the result.
- [x] Two unrelated Tasks with the same title remain distinct after convergence.
- [x] Deletion is a durable tombstone that stale earlier edits cannot reverse.
- [x] An imported deletion targeting the Task used by the local Current Session is retained but does not invalidate active work; it becomes effective when the local reference ends.
- [x] Task title snapshots already stored in Session History are never rewritten by synchronized Task changes.
- [x] Duplicate, stale, and differently ordered imports are idempotent.
- [x] Two-database scenarios cover offline edits, deterministic resolution, tombstones, same-title Tasks, and deferred deletion.
