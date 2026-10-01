# 01: Sync one Task through one file

**What to build:** Let two fresh Pomotui installations with independent local databases exchange one newly created Task through one provider-neutral `pomotui.sync` file. This tracer bullet establishes the smallest complete path from durable Task mutation, through the Timer Service and sync document, to import and CLI-visible state on another installation.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] A user can enable synchronization by selecting one local file path, request an immediate merge, and inspect basic status through both human-readable and JSON CLI output.
- [x] Creating a Task on one fresh installation produces a globally identified synchronization record without changing the Task identity exposed by existing local interfaces.
- [x] A second fresh installation can import the file and expose the same Task through its existing Task commands.
- [x] Reimporting the same file is idempotent and never creates a duplicate Task or record.
- [x] The sync document is provider-neutral, versioned, self-describing JSON with deterministic record ordering and stable serialization.
- [x] The Timer Service remains the sole writer of both local domain state and locally retained synchronization records.
- [x] The file is replaced atomically from a validated temporary document; it is never a SQLite database, database backup, or whole-service snapshot.
- [x] Two-service integration coverage demonstrates Task creation travelling through the complete database, service, protocol, CLI, file, and import path.
- [x] Existing behavior remains unchanged when synchronization is not enabled.
