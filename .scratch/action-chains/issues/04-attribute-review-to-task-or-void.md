# 04 — Attribute an unassigned review to a Task or Void

**What to build:** When a reviewed Focus Session had no Current Task, let the user assign a regular Task or the protected system Void Task during successful review. All history, totals, statistics, and chain data update together, and Void entries retain a meaningful Chain Entry Title.

**Blocked by:** 02 — Review a Task Session as successful.

**Status:** ready-for-agent

- [x] Migration and new-database setup provide exactly one system-owned Void Task whose title is always `Void`.
- [x] The Void Task cannot be renamed, completed as an ordinary Task, or deleted.
- [x] System Void identity is not inferred from title; an ordinary user Task titled `Void` remains an ordinary Task.
- [x] A Session without Task attribution cannot be reviewed until the user selects a regular Task or the Void Task.
- [x] Review can assign an existing regular Task or create/select a regular Task through the established Task identity rules.
- [x] Choosing Void requires a non-empty Chain Entry Title.
- [x] The regular Task or Void assignment, Session History attribution, Task focus time, statistics, Chain Link, and Pending Review clearing commit atomically.
- [x] An already attributed reviewed Session cannot be reassigned.
- [x] The Void Task remains visible in focus-time statistics and receives the reviewed Session's exact focus time.
- [x] Chain views use a regular Task title snapshot for ordinary entries and the Chain Entry Title for Void entries while retaining Void attribution.
- [x] `Void` is displayed unchanged in every interface language.
- [x] Migration tests preserve any pre-existing ordinary Task titled `Void` while creating the distinct singleton system Task.
- [x] Real-SQLite transaction, protocol, CLI, localized TUI, statistics, and rollback tests verify the complete flow.

