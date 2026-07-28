# Add Vim list navigation and visual multi-selection

Status: resolved

## Classification

Keyboard interaction feature.

Blocked by: 21

## Problem

Long Task and Session History lists only support one-row movement. Familiar
Vim navigation (`gg`, `G`, `u`, `d`) and visual selection (`v`) are missing,
making bulk review and deletion slow.

## Scope

- Support `gg` for first item and `G` for last item in the active list.
- Support `u` and `d` for page-up and page-down without conflicting with
  destructive actions.
- Support `v` to start/finish a contiguous visual selection in History, where
  records have a bulk action.
- Render the selection distinctly and expose its item count.
- Apply bulk-capable actions, beginning with History deletion, to the visual
  selection.
- Update Help/footer text and add deterministic input/render tests.

## Acceptance

- Navigation operates on Tasks in Dashboard and records in History; visual
  selection operates on History records.
- `gg`, `G`, `u`, and `d` clamp safely at list boundaries.
- `v`, movement, and `v`/Escape produce predictable selections.
- A destructive bulk action always requires confirmation and shows the count.
- Existing single-item shortcuts remain usable without ambiguity.

## Comments

Created from user feedback on 2026-07-28.

Resolved 2026-07-28. `gg`, `G`, `u`, and `d` navigate the active Task/History
list. History supports `v` contiguous selection and `D` confirmed deletion.
Help and footer hints document the interaction, with input/render regression
coverage.
