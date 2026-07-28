# Delete selected Session History entries

Status: resolved

## Classification

Privacy and data-management feature.

Blocked by: 20

## Problem

Session History is durable and currently append-only from every Timer
Frontend. Users need to remove mistaken, test, or private entries that they do
not want retained.

## Scope

- Give each Session History record a stable identity.
- Add an idempotent Timer Service command for deleting records by identity.
- Recompute summaries and Task focus totals from the remaining history.
- Add History selection and a confirmation prompt in the TUI.
- Preserve Task records and the Current Session.
- Persist deletion across Timer Service restart.
- Add domain/service/protocol/TUI tests.

## Acceptance

- A user can select one or more History records, request deletion, review a
  confirmation, and cancel or confirm it.
- Confirmed records disappear after refresh and restart.
- Today, seven-day, and per-Task totals no longer include deleted records.
- Deleting history never deletes Tasks or changes the Current Session.
- No record can be deleted accidentally by a stale visible row index.

## Comments

Created from user feedback on 2026-07-28.

Resolved 2026-07-28. Session History records now carry durable identities and
the Timer Service accepts idempotent deletion by identity. The TUI provides
single/visual selection and confirmation, while Today, Review, and Task totals
recompute from the remaining history.
