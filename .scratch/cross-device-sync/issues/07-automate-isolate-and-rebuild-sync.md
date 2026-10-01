# 07: Automate, isolate, and rebuild synchronization

**What to build:** Make single-file synchronization automatic and operationally safe, with recovery commands and health reporting that never put local timer operation or the local database at risk.

**Blocked by:** 01 / Sync one Task through one file.

**Status:** ready-for-agent

- [ ] The Timer Service attempts synchronization at startup, at a bounded periodic interval, and after a shareable local mutation.
- [ ] Synchronization work does not block timer progression, and a file failure does not fail an already durable local mutation.
- [ ] A user can disable automatic synchronization or change the configured path without deleting locally retained records.
- [ ] `sync rebuild` creates a validated file from all records known to the local database and clearly reports that it cannot recover unseen remote records.
- [ ] Rebuild and ordinary writes use a validated temporary document and atomic replacement while preserving the previous valid destination on failure.
- [ ] Missing, unreadable, unwritable, truncated, malformed, oversized, integrity-invalid, and unsupported-version files perform no database mutation and are not rewritten.
- [ ] An external replacement between read and write cannot make the local database forget a known record; a later retry restores the retained union.
- [ ] Sync status reports enabled state, path, attempts, last success, latest error or warning, known local records, readable file records, and format version without a Device registry or acknowledgements.
- [ ] Protocol and CLI behavior is covered for enable, disable, now, rebuild, status, JSON output, paths with spaces, and stable errors.
- [ ] Local timing, review, restart recovery, and reminders continue to operate while synchronization is unhealthy.

