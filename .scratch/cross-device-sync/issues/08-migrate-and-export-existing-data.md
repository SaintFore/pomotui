# 08: Migrate and export existing data safely

**What to build:** Let an existing Pomotui installation enable synchronization without losing or reinterpreting its current database, and export all previously durable Shared Activity into the new mergeable format.

**Blocked by:** None.

**Status:** resolved

**Milestone:** Issue 09 has frozen the stable data contract. This issue replaces the pre-freeze reset policy with supported migration; see ADRs 0009 and 0011.

- [x] Enabling synchronization on an existing database first creates a timestamped backup that can be restored independently of the sync file.
- [x] Database compatibility migration runs atomically when a supported legacy database is opened; the later global identity mapping, locally retained record creation, and initial projection changes run in a separate atomic first-export transaction.
- [x] Existing Tasks and their lifecycle state receive stable global mappings without changing local frontend-visible identities.
- [x] Existing Focus and Break Session History, outcomes, durations, Task attribution, snapshots, and every deletion state still durably and unambiguously represented by the legacy database export exactly once.
- [x] Existing submitted Reviews, current and Ended Chains, Chain Links, Chain Breaks, Chain Entry Titles, Reflections, and every chain deletion still durably and unambiguously represented by the legacy database export exactly once.
- [x] Existing Reward Milestones, unlock snapshots, availability, claims, and every deletion state still durably and unambiguously represented by the legacy database export exactly once.
- [x] Current Session, Pending Review, Focus Cycle, Session Durations, interface preferences, and transient frontend state remain local and are not written to the sync document.
- [x] The reserved title `Void` identifies only the singleton system Void Task; creating or renaming an ordinary Task to that exact normalized title is rejected.
- [x] Retrying an interrupted or completed migration is idempotent and never duplicates exported domain facts.
- [x] Independently migrated databases are treated as independent histories; coincidentally identical legacy records are not silently deduplicated.
- [x] Representative production-shaped migration fixtures prove preservation, backup creation, rollback, restart safety, and successful import into a fresh second database.

## Decisions

- Database compatibility migration occurs when the Timer Service opens a supported
  legacy database. The first `sync enable` performs the one-time export of legacy
  domain data into Synchronization Records; enabling synchronization is not the
  trigger for otherwise-required database compatibility work.
- Migration supports an explicit matrix of released legacy SQLite schema and
  persisted-state format versions. An unsupported combination is rejected before
  backup creation or mutation; migration never guesses missing fields or silently
  supplies semantic defaults.
- Preservation is measured against information the legacy database still durably
  contains. Migration exports every unambiguously recoverable fact and deletion
  state, but never invents facts that the legacy model did not retain. A database
  whose extant state cannot be represented faithfully is rejected.
- Exactly-once export means each legacy fact receives one permanently stable
  Synchronization Record identity. The migration marker, local-to-global identity
  mappings, and generated records commit in one database transaction, and retries
  reuse them. Completion belongs to the database and is independent of sync path.
- Before the first migration mutation, Pomotui creates a non-overwriting,
  timestamped, durable, transactionally consistent SQLite backup that is restorable
  without the sync file. Backup failure prevents migration from starting. Pomotui
  retains every such backup unless the user removes it explicitly.
- When first enabling synchronization with an existing file, Pomotui completely
  validates that file without mutation, creates the database backup, then commits
  legacy mappings, exported records, and the incoming projection in one database
  transaction. Only after the commit may it atomically replace the sync file. A
  replacement failure leaves all records retained locally for a safe retry.
- Synchronization records represent a reviewed Session's Task attribution as an
  explicit choice between the system Void Task and a regular Task global identity.
  `Void` is reserved for the system Void Task, so supported earlier records can
  upgrade that attribution without ambiguity. This contract change requires a
  new sync-document version and migration of supported earlier records.
- Migration does not invent historical mutation times. It captures one migration
  time and assigns a stable ordering to generated baseline versions. Original
  Session and Review business times remain unchanged, and retry reuses every
  persisted identity and time. A current legacy state is represented directly;
  lifecycle versions are synthesized only when required to express a recoverable
  deletion.
- If the database transaction commits but initial sync-file replacement fails,
  synchronization remains enabled and all records remain retained locally. Human
  and JSON responses distinguish successful local migration from failed initial
  publication, sync health records the filesystem error, and retry is safe.
- A migration backup is a consistent SQLite file that can directly replace the
  database while the Timer Service is stopped. The documented manual procedure is
  tested by opening an isolated service from the backup and verifying migration-
  prior state; this issue does not add a database-overwriting restore command.
- Pomotui creates a separate durable backup before each one-time database rewrite
  boundary, including schema migration and initial legacy export. Backup names
  identify their stage and timestamp, never overwrite an existing file, and are
  retained until the user explicitly removes them.
- Migration tests use checked-in, privacy-safe binary SQLite fixtures generated by
  every supported released Pomotui version. Each fixture records its originating
  version and scenario inventory; current code must not construct a facsimile of
  the legacy format at test runtime. Fixtures cover all representative states the
  originating format could durably retain, including the system Void Task.
- The supported inputs are the unversioned persisted state with SQLite schema 3
  released by both v0.1.0 and v0.2.0, plus persisted-state format 2 and sync-
  document format 4 frozen when issue 09 completed. Pre-freeze, unreleased
  persisted-state format 1 and sync-document formats 1 through 3 remain subject
  to the explicit reset policy and are not migration inputs. Unknown formats are
  rejected without mutation.
- The next sync-document version reads and losslessly upgrades format 4. Once one
  upgraded Device writes the new format, an older Device safely rejects it as a
  future version without modifying either the file or its local database. Local
  timer work remains available and is retained for convergence after that Device
  is upgraded; no Device registry, acknowledgement, or dual-format document is
  introduced.
