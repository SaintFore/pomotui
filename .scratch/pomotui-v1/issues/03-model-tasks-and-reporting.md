# Model Tasks, history, and reporting

Status: resolved
Blocked by: 01, 02

## Objective

Implement Task rules, durable history records, and pure Today/seven-day
projections.

## Scope

- Create, rename, complete, reopen, select, and delete Tasks by stable ID.
- Resolve exact titles and reject ambiguous matches with candidate IDs.
- Enforce deletion constraints for a Task referenced by Current Session.
- Record title snapshots and actual focus time in Session History.
- Compute daily totals, Completed Rounds, seven-day trend/average, and per-Task
  totals using an injected local-day boundary.

## Acceptance

- Rename/delete never rewrites historical snapshots.
- Completing a Task neither stops nor detaches Current Session.
- Unattributed Focus Sessions are valid.
- Projection tests cover midnight and timezone-boundary inputs.

## Comments

Implemented 2026-07-26 in `pomotui-domain`; public APIs cover stable identities,
ambiguous titles, Task lifecycle, immutable history snapshots, per-Task totals,
and caller-supplied local-day boundaries.
