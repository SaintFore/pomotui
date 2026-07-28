# Align TUI lists and separate Task selection from starting

Status: resolved

## Classification

Interaction and visual-fidelity issue.

## Problem

Terminal columns drift when Task titles, Settings labels, and Session History
contain Simplified Chinese because layout uses character counts or hard-coded
spaces instead of terminal display widths. Task confirmation with Enter also
starts a Focus Session immediately, preventing users from selecting a Task
without starting it.

## Scope

- Align Task accumulated times for Latin and wide-character titles.
- Align Settings labels, controls, and values in both interface languages.
- Redesign Session History as a clearer, consistently aligned list.
- Make Task selection independent from starting a Session.
- Update visible keyboard guidance to match the interaction.
- Add deterministic rendering and input regression tests.

## Acceptance

- Task time values begin in the same terminal column regardless of title
  language or width.
- Settings values begin in one stable column in English and Simplified Chinese.
- Session History presents kind, outcome, Task, and duration in stable columns,
  with a readable compact fallback.
- Moving to or confirming a Task never emits a Start command.
- Space starts a pending Focus Session with the currently selected Task.
- Wide/narrow and English/Simplified Chinese tests cover the corrected behavior.

## Comments

Created from user screenshots and interaction feedback on 2026-07-28.

Resolved 2026-07-28. TUI layout now measures Unicode terminal display width
for truncation and padding. Tasks and Settings use stable value columns;
History uses a labeled four-column layout on wide terminals and compact
two-line records on narrow terminals. Enter confirms the selected Task without
starting, while Space starts a pending Focus Session with that selection.
Focused rendering and input tests reproduce all four reported symptoms and
protect the corrected behavior.
