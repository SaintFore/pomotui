Status: ready-for-agent

# Provider-neutral cross-device synchronization

## Problem Statement

Pomotui users may work from several computers, but each installation currently keeps an independent SQLite database. Completed work, Task totals, Session History, Action Chains, and reward progress therefore describe only the computer currently in use. A user cannot answer how much time they spent or what they accomplished across all their computers without manually reconciling several databases.

The target user normally uses one computer at a time, already has a preferred file-replication tool, and does not want an account, hosted service, or permanently online machine. Synchronizing the SQLite database itself is unsafe because each Timer Service writes a whole state snapshot, existing identities are local numeric counters, and arbitrary file-replication tools do not coordinate concurrent database writers.

## Solution

Pomotui will converge durable Shared Activity through one user-selected `pomotui.sync` file. The file is a provider-neutral, versioned, mergeable exchange document; Pomotui neither knows nor cares whether it is transported by Syncthing, Dropbox, iCloud Drive, Nextcloud, a USB drive, or another file-copy mechanism.

Each local SQLite database remains a durable replica of every synchronization record that installation has observed. The Timer Service reads the sync file, validates it completely, unions its records with the locally retained records, imports newly observed records transactionally, and atomically writes the merged union back. The sync file is a disposable exchange artifact rather than the database or sole backup and can be rebuilt from the records known to one local database.

Shared Activity includes ended Sessions, submitted Session Reviews, Tasks, Session History, Action Chains, Reflections, Chain Entry Titles, Reward Milestones, reward claims, and synchronized deletion facts. Current Sessions, Pending Reviews, Focus Cycle progress, Session Durations, interface preferences, and transient frontend state remain local to each Device.

The design is deliberately eventually consistent. A record temporarily lost from the sync file can be reintroduced by any local database that previously observed it. No design without another replica can recover a record that existed only on a permanently lost Device and was never imported elsewhere.

## User Stories

1. As a user with several computers, I want completed work from all of them to converge, so that my history describes my work rather than one machine.
2. As a user, I want one sync file, so that I can move it with the file-replication software I already trust.
3. As a user, I want to select the sync file independently on each computer, so that its local path can match that computer's filesystem layout.
4. As a user, I want Pomotui to remain unaware of the transport provider, so that changing from Syncthing to another tool requires no Pomotui migration.
5. As a user, I want synchronization without an account, so that a personal local application does not acquire account management.
6. As a user, I want synchronization without a hosted or self-hosted service, so that no machine must remain online.
7. As a user, I want each Device to retain its own Current Session, so that offline timing remains reliable.
8. As a user, I want each Device to retain its own Focus Cycle, so that activity imported from another Device does not change this Device's break rhythm.
9. As a user, I want Running and Paused Sessions to remain local, so that synchronization never takes control of an active timer.
10. As a user, I want a Pending Session to remain local, so that another Device cannot replace the next Session I have already selected.
11. As a user, I want a Pending Review to remain on the Device where it arose, so that an unfinished judgment is not mistaken for completed Shared Activity.
12. As a user, I want an ended Session to enter Shared Activity, so that completed, stopped, and skipped Session History converges even when review state differs.
13. As a user, I want Break Sessions in shared Session History, so that history remains complete while imported breaks still do not advance my local Focus Cycle.
14. As a user, I want submitted Session Reviews to synchronize, so that successful and failed work contributes to one cross-device Action Chain history.
15. As a user, I want Tasks to synchronize, so that I can select and continue the same work from another computer.
16. As a user, I want same-titled Tasks to retain distinct identities, so that synchronization does not merge unrelated work merely because titles match.
17. As a user, I want Task creation, rename, completion, reopening, and deletion to converge, so that Task state eventually agrees everywhere.
18. As a user, I want imported Session History to retain its original Task identity and title snapshot, so that later Task edits do not rewrite history.
19. As a user, I want total focus time to be derived from converged Session History, so that importing the same record twice never doubles my totals.
20. As a user, I want Reflections and Chain Entry Titles to synchronize, so that later review work is available everywhere.
21. As a user, I want Reward Milestones to synchronize, so that all Devices evaluate the same promises against the shared Action Chain.
22. As a user, I want reward claims to synchronize, so that a reward claimed on one Device is not offered again elsewhere.
23. As a user, I want a claimed reward never to be revoked by a late record, so that software does not deny a real-world action that already occurred.
24. As a user, I want unlocked but unclaimed rewards to follow the latest projected Action Chain, so that late Shared Activity can correct current eligibility.
25. As a user, I want every ended Session and submitted Review from two offline Devices to be retained, so that concurrent work is not resolved by discarding one side.
26. As a user, I want submitted Reviews to have one deterministic Review Order, so that every Device projects the same Action Chains after seeing the same records.
27. As a user, I want Review Order to use the source Session's recorded end time with a stable identity tie-breaker, so that file arrival order cannot change chain history.
28. As a user, I want a warning when imported timestamps are implausibly far from local time, so that clock mistakes do not remain invisible.
29. As a user, I accept that a late offline Review may revise past Action Chain boundaries, so that offline operation does not require a server-assigned order.
30. As a user, I want mutable records to resolve deterministically, so that ordinary simultaneous edits do not require a device-management or conflict-resolution workflow.
31. As a user, I want a later valid edit to supersede an incorrectly ordered edit, so that I can repair a result caused by clock skew.
32. As a user, I want deletions represented as durable synchronization records, so that an older file cannot silently resurrect deleted content.
33. As a user, I want imported deletion of a Task used by a local Current Session to wait until the Session no longer references it, so that synchronization cannot invalidate active work.
34. As a user, I want deleting Session History not to delete related Action Chain records, so that existing domain history rules remain intact across Devices.
35. As a user, I want synchronization records to have globally unique identities, so that local numeric ID collisions cannot conflate unrelated data.
36. As an existing user, I want my current database backed up before synchronization migration, so that I can recover if migration fails.
37. As an existing user, I want all existing durable history exported when I enable synchronization, so that the feature starts with my past work rather than only future work.
38. As an existing user, I want migration to preserve my current Tasks, Sessions, Action Chains, rewards, settings, and Current Session, so that enabling synchronization is not a reset.
39. As a user, I want migration failure to roll back completely, so that enabling synchronization cannot leave a partially upgraded database.
40. As a user, I want malformed or incomplete sync files rejected before database writes begin, so that a damaged transport artifact cannot corrupt local data.
41. As a user, I want an unsupported future sync-file version rejected without rewriting it, so that an older Pomotui cannot destroy newer data.
42. As a user, I want sync-file writes to be atomic, so that a crash or transport scan during writing cannot observe a half-written document.
43. As a user, I want repeated import of the same record to be idempotent, so that file replication and service restarts cannot duplicate work.
44. As a user, I want locally known immutable records never removed merely because they are absent from the current sync file, so that an older remote copy cannot erase history.
45. As a user, I want synchronization failure isolated from timer operation, so that an inaccessible file never prevents starting, pausing, completing, or reviewing local work.
46. As a user, I want local mutations to remain durable when the sync file is unavailable, so that they can be exported after access returns.
47. As a user, I want Pomotui to merge automatically at startup, periodically while running, and after relevant local mutations, so that normal use requires no manual import step.
48. As a user, I want a manual sync command, so that I can request an immediate read, merge, and write when diagnosing file transport.
49. As a user, I want synchronization status to report the configured path, last successful merge, last error, and file format version, so that failures are understandable without exposing device-management concepts.
50. As a user, I want to disable automatic synchronization without deleting local records, so that changing transport tools is safe.
51. As a user, I want to change the configured sync-file path, so that I can move between file-replication tools.
52. As a user, I want to rebuild the sync file from this database's known records, so that a deleted, overwritten, or damaged exchange file is recoverable.
53. As a user, I want rebuild to validate a temporary document before replacing the target, so that recovery cannot make the situation worse.
54. As a user, I want rebuild to state that it only contains records known locally, so that I do not mistake it for recovery of unseen records.
55. As a user, I want the sync file to remain a single self-contained document, so that I can copy, inspect, back up, or replace one artifact.
56. As a user, I want the first version to favor correctness over compaction, so that old offline records are not lost by premature garbage collection.
57. As a user, I want synchronization to preserve existing CLI, TUI, tray, and Waybar behavior, so that the feature does not require changing how I use the timer.
58. As a CLI user, I want synchronization management available through stable commands and JSON output, so that I can automate setup and diagnosis.
59. As a user, I want human-readable errors for permission, format, validation, and atomic-replacement failures, so that I know whether to fix Pomotui or my file-sync setup.
60. As a user, I want the documentation to explain that `pomotui.sync` is not a backup, so that I preserve local database backups when retiring a computer.

## Implementation Decisions

- The Timer Service remains the sole writer of the local domain database. Timer Frontends request synchronization operations through the versioned service protocol and never read or write SQLite or the sync file directly.
- A synchronization component will sit behind one service-level interface that can read and atomically replace a sync document. The platform layer implements filesystem access; merge and projection rules remain independent of filesystem and transport-provider APIs.
- The sync file is one self-describing, versioned JSON document named `pomotui.sync` by convention. A user may choose another filename and a different local path on every Device.
- The document contains a format version, integrity metadata, and a deterministically ordered collection of synchronization records. Stable serialization makes unchanged unions byte-stable and avoids needless file-replication traffic.
- The file is not SQLite, a SQLite backup, or a serialized whole-service snapshot. No file-replication provider is imported as a dependency or named in the protocol.
- Each synchronization record and every synchronized entity receives a globally unique random identity. Existing local numeric identifiers remain valid inside a migrated database but are mapped to global identities before exchange.
- Synchronization records are append-only facts. Immutable domain facts are never edited in place. Changes to mutable entities are new version records that reference the stable entity identity.
- The local database stores every valid record it creates or imports. Absence from a later sync file is never interpreted as deletion. There is no synchronization-record garbage collection or sync-file compaction in this version.
- Merge is set union by record identity followed by deterministic projection. Importing and merging the same document repeatedly produces the same database state and output document.
- Mutable entity versions are ordered by their recorded mutation time and then by globally unique record identity. The greatest valid version wins. A later user edit can supersede an undesirable result.
- Deletion is a tombstone record. A valid tombstone permanently dominates earlier versions of the same entity; stale edits cannot resurrect it. Domain commands continue to enforce which entities may be deleted and what historical snapshots remain.
- If an imported Task tombstone targets a Task referenced by the local Current Session, the record is retained but its local effect is deferred until the reference ends. The active Session retains its Task snapshot and synchronization never terminates or rewrites it.
- Ended Focus and Break Sessions enter Shared Activity independently of Session Review. A Pending Review remains local; its later submitted Review becomes an additional shared record referring to the already shared Session.
- Current Session state, Pending Review state, Focus Cycle progress, default Session Durations, themes, locale, notification and sound preferences, keybindings, frontend navigation, selection, and cursor state never enter the sync document.
- Imported Completed Rounds contribute to cross-device Session History and statistics but do not advance the importing Device's Focus Cycle. Each Device's local Focus Cycle only observes its own Session transitions.
- Review Order is the ascending order of source Session end instant followed by Review identity as a stable tie-breaker. The same set of records therefore produces the same Action Chains regardless of file arrival order.
- Time values are UTC instants. An implausible imported clock difference is reported as a non-blocking health warning; it does not cause silent record rejection. Corrective edits produce later version records rather than rewriting exchanged facts.
- Action Chain projections, Reward Milestone eligibility, and unclaimed unlock availability are recomputed after relevant imports. A claim is an immutable historical fact and remains claimed even if later Shared Activity changes the projection that originally authorized it.
- Duplicate unlock facts caused by disconnected Devices are collapsed deterministically for the same Reward Milestone and projected Action Chain. Claims referencing any collapsed duplicate remain represented by the surviving historical claim state.
- Task focus totals and aggregate statistics are projections of globally identified Session records, not independently merged counters.
- Existing whole-chain and Session History deletion behavior is represented by synchronized tombstones and continues to preserve or remove related information according to the current domain rules.
- Enabling synchronization performs a versioned SQLite migration in one transaction after creating a timestamped backup. The migration adds global identity mappings and durable synchronization-record storage without replacing the existing database file.
- Initial export converts all existing shareable state into synchronization records. Independently migrated databases are treated as independent sources; automatic semantic deduplication of coincidentally identical legacy records is not attempted.
- The sync file is parsed, size-bounded, structurally validated, version-checked, and integrity-checked before an import transaction begins. A failed validation performs no database mutation and does not rewrite the file.
- Database import is atomic. Projection failure rolls back newly observed records and all derived changes together.
- Output uses a temporary file in the target directory, flushes it, and atomically replaces the destination only after complete serialization and validation. A concurrent external replacement may temporarily win, but subsequent union with retained local records restores convergence.
- Automatic synchronization occurs on Timer Service startup, at a bounded periodic interval, and after a shareable local mutation. Synchronization runs outside the timer progression critical path; failures update health state without failing the already durable local mutation.
- The service protocol gains operations to enable synchronization with a path, disable automatic synchronization, request an immediate merge, rebuild the file from locally known records, and query status. Mutation requests use the existing idempotency mechanism.
- The CLI exposes `sync enable`, `sync disable`, `sync now`, `sync rebuild`, and `sync status`, including stable JSON responses. No Device-list, acknowledgement, handoff, login, or conflict-resolution commands are introduced.
- `sync rebuild` ignores the content of an invalid target, serializes all records known to the local database into a temporary document, validates it, and atomically replaces the target. Its result explicitly reports that unseen remote records cannot be reconstructed.
- Changing or disabling the configured path does not delete synchronization records from SQLite. Re-enabling at another path can rebuild or merge using the same locally known set.
- Sync status contains only the configured path, enabled state, last attempted merge, last successful merge, latest error or warning, local known-record count, file record count when readable, and file format version. Pomotui maintains no Device registry or replication acknowledgements.
- Sync configuration is device-local. It is stored with other local settings or service configuration and is never synchronized through the file it names.
- Unknown record kinds within a supported extensible envelope are retained only when their integrity can be validated and their preservation semantics are defined by the format version; otherwise the whole unsupported document is left untouched for a newer Pomotui.
- The user guide will document setup with a generic synchronized path, provider-neutral operation, concurrent-overwrite recovery, rebuild behavior, clock-skew effects, local backup requirements, and the fact that the sync file is not proof of replication.

## Testing Decisions

- Tests assert externally observable convergence, domain state, durability, and failure isolation rather than private helper calls or a particular internal record layout. Format compatibility and migration tests may assert the public persisted schema because compatibility is their external contract.
- The primary inexpensive seam is a pure synchronization model supplied with two sets of records. Property-style scenario tests verify union is commutative, associative, and idempotent and that deterministic projection does not depend on import order.
- Domain scenario tests cover two offline Devices producing disjoint ended Sessions and Reviews, later convergence, deterministic Review Order, late failure insertion, Action Chain reprojection, and unchanged per-Device Focus Cycles.
- Domain tests cover mutable Task and Reward Milestone versions, stable tie-breaking, permanent tombstones, same-title distinct Tasks, corrective later edits, duplicate reward unlock collapse, and immutable claims after reprojection.
- Service integration tests with real SQLite are the main durability seam. They verify local mutation plus record creation is atomic, import plus projection is atomic, retries do not duplicate records, and restart preserves every known synchronization record.
- Migration tests upgrade representative current databases containing Tasks, attributed and unattributed Session History, Pending Review, current and Ended Chains, Reward Milestones, unlocks, claims, and deletions. They verify preservation, global identity mapping, initial export, backup creation, rollback on injected failure, and retry safety.
- Migration tests verify that Current Session, Pending Review, Focus Cycle, Session Durations, and interface settings remain local and do not appear in the sync document.
- Filesystem adapter tests use temporary directories to verify temporary-file creation, flush and atomic replacement, same-directory replacement, permission errors, missing parents, interrupted writes, and preservation of the previous valid file on failure.
- Sync-document contract tests cover deterministic serialization, supported versions, integrity failure, truncation, invalid JSON, duplicate identities, conflicting payloads under one identity, excessive document size, unknown versions, and byte-stable output for an unchanged record set.
- Two-replica integration scenarios start from separate SQLite databases and exchange one file in alternating orders. They verify eventual equality after both replicas observe the union and verify that an older file cannot make either database forget a known immutable record.
- Recovery scenarios overwrite or delete the file, rebuild it from one database, then allow another database with additional records to merge. They verify the final file and both databases contain the union while rebuild never claims to recover unseen records.
- Race-oriented adapter tests replace the sync file between read and write. They verify no database loss, a valid atomic output, and convergence on a later retry rather than promising impossible cross-process locking with arbitrary transport software.
- Service tests verify a malformed, unreadable, unsupported, or unwritable sync file changes only sync health state and never prevents local timer commands or durable local mutations.
- Service tests verify imported Task deletion is deferred while that Task is referenced by a local Current Session and becomes effective after the reference ends without rewriting Session History.
- Service tests verify imported Completed Rounds affect shared statistics but not the local Focus Cycle or current Pending Session recommendation.
- Reward tests verify a claim remains historical after a late failure Review changes chain boundaries, while unclaimed eligibility follows the latest deterministic projection.
- Protocol tests cover every sync command, version compatibility, idempotent mutation keys, status fields, validation errors, and JSON-stable identifiers.
- CLI tests cover enable, disable, now, rebuild, and status; human-readable error messages; stable JSON output; paths containing spaces; relative-path normalization; and rebuild warnings.
- Process-level end-to-end tests run two Timer Service instances with isolated databases and one exchange file. They create work independently, alternate file access, restart services, and prove converged Session History, Task totals, Review Order, Action Chains, Reward Milestones, and claims while Current Sessions and Focus Cycles remain independent.
- Regression tests retain existing Timer Service sole-writer, snapshot recovery, reminder, Task ambiguity, Session History, Action Chain, reward, CLI, TUI, tray, and Waybar behavior when synchronization is disabled.
- The completed implementation must pass formatting, linting, the full workspace test suite, and the repository's process-level end-to-end suite.

## Out of Scope

- Synchronizing the SQLite database file, its backup, WAL, or journal files.
- Accounts, authentication, hosted synchronization, peer discovery, direct device-to-device networking, or a permanently online coordinator.
- Integration code for Syncthing, Dropbox, iCloud Drive, Nextcloud, or any other transport provider.
- A Device registry, Device list, replication acknowledgements, primary-Device election, ownership token, handoff workflow, or “safe to retire” guarantee.
- Synchronizing Running, Paused, or Pending Sessions; controlling another Device's timer; or moving a Pending Review between Devices.
- Synchronizing Focus Cycle progress, Session Durations, themes, locale, sounds, notifications, keybindings, or transient frontend state.
- Strong consistency, real-time delivery, proof that another Device received a record, or recovery of a record that no surviving local database observed.
- Treating `pomotui.sync` as a replacement for normal backups.
- End-to-end encryption, per-Device signing, access control, or protection from a malicious process that can rewrite both the sync file and local application data.
- Automatic semantic deduplication of pre-existing records from independently migrated databases.
- Interactive conflict resolution for mutable fields; deterministic ordering is used instead.
- Sync-file compaction, synchronization-record garbage collection, incremental multi-file logs, or splitting the exchange artifact into per-Device files.
- TUI synchronization settings or dashboards in the first release; CLI management and existing views reflecting imported domain state are sufficient.

## Further Notes

- This specification applies ADR 0001 by keeping each Timer Service authoritative for its own Current Session, ADR 0003 by retaining SQLite as local durable domain storage, and ADR 0008 by treating one provider-neutral file as a mergeable exchange artifact.
- The highest practical test seam is two real Timer Service instances with independent SQLite databases exchanging one file. Most combinatorial merge behavior belongs below that seam in pure scenario tests; frontend tests should only validate the sync command contract.
- Existing persistence primarily stores a whole service snapshot with local numeric counters. The implementation must introduce durable synchronization records and global identity mappings without weakening existing recovery or sole-writer guarantees.
- The single-file design intentionally favors a small mental model over optimal write amplification. The format should reserve versioning space for future compaction without implementing it now.
- “Rebuild” means “recreate the exchange document from everything this local database knows,” not “recover every record ever created on every computer.”
