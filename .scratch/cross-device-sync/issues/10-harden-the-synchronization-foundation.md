# 10: Harden the synchronization foundation

**What to build:** Establish a validated synchronization format, a focused Sync Engine, and an explicit safe reset path so later automatic synchronization can evolve without impossible records, hidden data loss, or filesystem concerns leaking into the Timer Service.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Every synchronization record uses a kind-specific payload, validated global record and entity identities, and a validated UTC mutation instant; malformed combinations fail complete-document validation before any database write.
- [x] A protocol-neutral Sync Engine owns the document contract, validation, set union, and deterministic projection planning without depending on the Timer Service, SQLite, or platform filesystem APIs.
- [x] The Timer Service remains the sole durable database writer and maps global identities to local persistence identities when it applies a projection plan transactionally.
- [x] The existing Task lifecycle converges through the new format with deterministic byte-stable output and no regression in create, rename, complete, reopen, tombstone, or active-session deferral behavior.
- [x] `pomotui reset --all-data --confirm` runs only while the Timer Service is unreachable, creates and flushes a timestamped backup, removes the database and SQLite sidecars, preserves configuration, and never removes the configured sync file.
- [x] Unsupported pre-release data fails with an actionable reset instruction and is never ignored, partially imported, or deleted automatically.
- [x] Contract, property-style merge, real-SQLite service, reset, and isolated two-Device tests cover invalid records, deterministic convergence, restart durability, and preservation boundaries.
- [x] Domain documentation and the user-facing experimental compatibility guidance describe the validated record model and full-data reset policy.

## Comments

- Verified on `think` with two isolated Timer Service processes and one exchange file: Task convergence produced format v2, reset was rejected while the service was reachable, and offline reset created a durable backup while preserving the exchange file.
