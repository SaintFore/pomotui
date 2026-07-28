# Delete selected Tasks and scroll full History

Status: resolved

## Classification

Interaction and data-access issue.

## Problem

A Task that was used by the current pending Focus Session cannot be deleted,
even though no Session is running. The History view also receives only five
records and has no scrolling state, so older Session History is inaccessible.

## Scope

- Allow deletion of the Task referenced by a pending Session by safely
  detaching it first.
- Continue rejecting deletion of a Task referenced by a running or paused
  Session.
- Expose the complete Session History to the TUI.
- Add History-specific scrolling with keyboard navigation and visible guidance.
- Clamp scrolling when snapshots or terminal dimensions change.
- Add service and TUI regression tests.

## Acceptance

- A stopped/pending Task can be deleted and disappears from the Task list.
- Running or paused Session attribution cannot be invalidated by deletion.
- The History view can reach every available record.
- History scrolling works in wide and narrow layouts without moving the Task
  selection.
- Tests cover pending deletion, full-history projection, and scrolling.

## Comments

Created from installed-product feedback on 2026-07-28.

Resolved 2026-07-28. Pending Sessions now release their Task reference before
deletion, while running and paused Sessions retain deletion protection. The
service projects complete Session History, and the TUI provides bounded
History-specific scrolling with record-range and keyboard guidance. Service
tests cover both deletion states and full projection; a narrow-layout TUI test
proves older records are reachable without changing Task selection. Delete is
available through the discoverable lowercase `d` shortcut as well as `D`.
