# Model Sessions and the Focus Cycle

Status: resolved
Blocked by: 01

## Objective

Implement the pure domain state machine for Current Session, Session Outcome,
Task attribution, and Focus Cycle recommendations.

## Scope

- Model Pending, Running, and Paused Sessions.
- Implement start, pause, resume, stop, skip, and deadline transitions.
- Preserve planned duration and calculate actual duration from injected time.
- Implement completed/stopped/skipped rules and the skipped-Focus-round case.
- Produce domain events/effects without performing persistence or notifications.
- Add scenario and property-style invariant tests.

## Acceptance

- No transition can create two Current Sessions.
- Only a deadline-completed Focus Session increments Completed Rounds.
- Stop preserves the same recommendation; skip advances to the following
  recommendation without starting it.
- Exhaustive tests cover Focus, Short Break, Long Break, and cycle reset.

## Comments

Implemented 2026-07-26 in `pomotui-domain`; six public state-machine scenarios
cover completion, pause/resume, stop, skip, both break kinds, and cycle reset.
