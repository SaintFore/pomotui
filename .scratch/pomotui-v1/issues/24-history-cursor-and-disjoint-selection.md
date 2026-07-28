# Show a History cursor and support disjoint selection

Status: resolved

## Classification

Keyboard interaction and selection-visibility issue.

## Problem

History navigation only scrolls content and does not render a strong cursor, so
the active record is unclear. Visual mode selects only contiguous ranges, while
users also need to delete several unrelated records.

## Scope

- Separate the active History cursor from its viewport offset.
- Render the cursor with the same full-row emphasis used by the Task list.
- Use Space to toggle the cursor record in a persistent disjoint selection,
  matching yazi-style selection.
- Accept Alt+Space as an alias where the terminal reports the modifier.
- Keep `v` contiguous visual selection available.
- Delete marked records when marks exist, otherwise delete the cursor/visual
  selection.
- Update count hints, Help, and regression tests.

## Acceptance

- Exactly one visible History row has an obvious cursor while records exist.
- Cursor movement keeps the active record visible and scrolls only as needed.
- Space and Alt+Space independently toggle unrelated History records.
- Marked records remain visibly marked while navigating.
- Confirmed deletion contains precisely the marked identities.
- Empty and refreshed History clamp cursor, viewport, and stale marks safely.

## Comments

Created from user screenshot and selection feedback on 2026-07-28.

Resolved 2026-07-28. History now maintains an independent cursor and viewport,
renders the cursor as a full-width highlighted row, and displays checkmarks for
marked records. Space and Alt+Space toggle arbitrary records; deletion prefers
the marked identity set while retaining `v` range selection as a fallback.
Rendering, non-contiguous selection, and exact deletion identities are covered
by regression tests.
