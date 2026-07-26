# Implement SQLite persistence and migrations

Status: resolved
Blocked by: 02, 03

## Objective

Persist all domain data transactionally behind repository ports.

## Scope

- Add versioned schema/migrations for Current Session, Focus Cycle, Tasks,
  Session History, recovery observations, idempotency keys, and reminders.
- Implement atomic load/transition/save operations as the sole-writer service.
- Store Task identity/title snapshots and planned/actual durations.
- Add corruption, incompatible-version, rollback, and migration tests.

## Acceptance

- Restart round-trips every valid domain state.
- A failed transaction leaves no partial transition.
- Replaying an idempotency key cannot repeat a mutation.
- Migration failure is diagnostic and never silently recreates the database.

## Comments

Implemented 2026-07-26 in `pomotui-platform`; schema v1 contains all specified
stores. Transaction, idempotency, rollback, migration, and incompatible-schema
tests pass against real bundled SQLite.
