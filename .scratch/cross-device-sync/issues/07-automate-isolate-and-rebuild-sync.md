# 07: Automate, isolate, and rebuild synchronization

**What to build:** Make single-file synchronization automatic and operationally safe, with recovery commands and health reporting that never put local timer operation or the local database at risk.

**Blocked by:** 10 / Harden the synchronization foundation.

**Status:** resolved

- [x] One named worker performs synchronization outside the Timer Service state lock and stops gracefully with the service.
- [x] Startup, every relevant durable local mutation, and a fixed 30-second interval submit requests through a bounded channel that coalesces overlapping work into one later run.
- [x] The worker reads and validates the document, the Timer Service remains the sole database writer while applying the plan transactionally, and the worker atomically writes the latest retained union.
- [x] A file failure updates synchronization health without degrading local durable health, failing timer progression, or undoing an already durable mutation.
- [x] A user can disable automatic synchronization or change the configured path without deleting locally retained records.
- [x] `sync rebuild` creates a validated file from all records known to the local database and clearly reports that it cannot recover unseen remote records.
- [x] Rebuild and ordinary writes use a validated temporary document and atomic replacement while preserving the previous valid destination on failure.
- [x] Missing, unreadable, unwritable, truncated, malformed, oversized, integrity-invalid, and unsupported-version files perform no database mutation and are not rewritten.
- [x] Before replacement the worker compares the source fingerprint, re-reading and replanning a changed document at most three times before leaving repair to a later trigger.
- [x] An external replacement between read and write cannot make the local database forget a known record; a later trigger restores the retained union.
- [x] Sync status reports enabled state, path, in-progress state, attempt start time, success only after import and replacement complete, stage-specific errors or warnings, known local records, readable file records, and format version without a Device registry or acknowledgements.
- [x] Protocol and CLI behavior is covered for enable, disable, now, rebuild, status, JSON output, paths with spaces, and stable errors.
- [x] Local timing, review, restart recovery, and reminders continue to operate while synchronization is unhealthy.

## Comments

- Implemented a bounded single-flight `pomotui-sync-worker`, fail-closed ordinary synchronization, explicit disable/rebuild operations, bounded source rechecks, and stage-specific health reporting. Rebuild is the only operation that creates a missing exchange file and warns that unseen remote records are unrecoverable.
