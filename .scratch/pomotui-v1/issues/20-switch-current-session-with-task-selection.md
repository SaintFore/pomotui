# Keep Current Session consistent with Task selection

Status: resolved

## Classification

Interaction and domain-consistency issue.

## Problem

Selecting another Task changes the TUI cursor but leaves the Current Session
attributed to the previous Task. The mismatch is surprising and can also make
the newly selected Task appear impossible to delete because the protected Task
is not visually obvious.

## Scope

- Make Enter select a Task for the Current Session, not merely move a cursor.
- Reassign a Pending Focus Session immediately.
- When a Running or Paused Focus Session belongs to another Task, ask before
  ending it and switching attribution.
- Preserve elapsed time from an ended Running or Paused Session in Session
  History.
- Keep Break Sessions unattributed.
- Show both cursor selection and Current Task clearly.
- Add domain/service/TUI regression tests.

## Acceptance

- Selecting a Task while Focus is Pending makes it the Current Task.
- Selecting a different Task while Focus is Running or Paused opens a
  confirmation instead of silently changing historical attribution.
- Confirming records the old Focus Session with its elapsed time and leaves a
  Pending Focus Session attributed to the selected Task.
- Cancelling leaves both the Current Session and its Task unchanged.
- A Task not referenced by the Current Session can be deleted normally.

## Comments

Created from user feedback on 2026-07-28.

Resolved 2026-07-28. Enter now binds a Pending Focus Session to the selected
Task. A Running or Paused Session prompts before switching; confirmation stops
and records the old Session before binding the new Pending Focus Session.
Service and TUI regression tests cover both paths.
