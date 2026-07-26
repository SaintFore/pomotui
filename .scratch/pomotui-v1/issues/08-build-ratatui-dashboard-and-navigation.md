# Build the Ratatui Dashboard and navigation

Status: resolved
Blocked by: 06

## Objective

Implement the production Timer First TUI with responsive layouts and complete
keyboard, palette, settings, and mouse interaction.

## Scope

- Build wide/narrow Dashboard, Today, Session History, and settings views.
- Render Tasks, countdown, Current Task/time, Focus Cycle, and next Pending
  Session with the validated hierarchy.
- Implement `j/k`, `h/l`, arrows, uppercase `K`, command palette, help, and
  basic mouse targets.
- Implement both signature themes and semantic state tokens.
- Show disconnected/reconnecting state without locally advancing time.

## Acceptance

- Fixed-size snapshots cover all states/themes listed in spec criterion 7.
- Narrow Today/History are separate views and narrow settings is full-screen.
- Every action is reachable through discoverable keyboard UI.
- Two TUI instances remain consistent through the Timer Service.

## Comments

Implemented 2026-07-26. The Timer First Dashboard consumes authoritative
Task/Today/History projections, uses responsive wide/narrow views, both
signature themes, semantic Focus/Break/Paused/Pending colors, disconnected
state, keyboard/arrow navigation, executable command palette, settings/help,
and mouse Session controls. Fixed-size TestBackend matrices cover all required
states, themes, layouts, settings behavior, and disconnection.
