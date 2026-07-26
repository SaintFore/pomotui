# Add configuration and completion animation

Status: resolved
Blocked by: 08

## Objective

Load validated TOML preferences and play the data-driven completion transition
without coupling it to Timer Service state.

## Scope

- Define documented defaults for durations, themes, keybindings, reminder
  preferences, sound, and optional animation path.
- Implement the built-in animation and custom `frame_ms`/`hold_frames`/`---`
  file adapter behind one elapsed-time interface.
- Validate timing, frames, terminal-safe text, and bounded dimensions.
- Fall back visibly to the built-in animation on invalid custom input.
- Reveal the authoritative Pending Session after playback.

## Acceptance

- Missing config uses documented defaults; invalid fields identify their path.
- Animation playback never sends a second completion or delays service state.
- Parser/limit/fallback behavior is fully tested.
- TUI tests prove the next Pending Session never starts automatically.

## Comments

Implemented 2026-07-26. Strict TOML defaults/validation cover durations, cycle,
theme, configurable keys, reminders, sound/volume, and animation path. The
fixed-canvas animation parser enforces timing, count, dimensions, and
terminal-safe text; invalid files visibly fall back. Running-to-Pending
playback is frontend-only and tests prove it never starts the Pending Session.
