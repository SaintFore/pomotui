# 05 — Bound and diagnose Session Reminder retries

**What to build:** Retry transient Session Reminder failures predictably, stop
permanent failures from running forever, and let Timer Frontends show whether
effects are pending, retrying, delivered, or exhausted.

**Blocked by:** 04 — Persist and recover Session Reminder delivery.

**Status:** resolved

- [x] Failed notification and sound effects retry with documented bounded
      backoff using a controllable clock.
- [x] Both attempt and age limits prevent indefinite or unexpectedly late
      delivery.
- [x] Exhausted effects remain available as terminal diagnostic records rather
      than disappearing.
- [x] Protocol diagnostics expose aggregate pending, retrying, delivered, and
      exhausted counts without putting the complete journal in every Snapshot.
- [x] CLI JSON and human-readable output distinguish each delivery state.
- [x] Tests do not sleep in wall-clock time and cover retry scheduling,
      independent effects, exhaustion, restart, and duplicate-minimization
      boundaries.

## Comments

Implemented deterministic bounded exponential retry with per-effect jitter,
three-attempt exhaustion, and a one-hour maximum delivery age. Pending effects
are only selected when due; failures retain a safe diagnostic and exhausted
rows remain durable. Snapshots expose pending, retrying, delivered, and
exhausted aggregates. Human and JSON CLI status, Waybar tooltip, and the TUI
surface retry/exhaustion state without shipping the delivery journal in every
Snapshot. Tests use explicit service/repository times rather than sleeping and
cover scheduling, attempt exhaustion, age exhaustion, restart, independent
effects, and frontend rendering. Workspace lint, all tests, and process
end-to-end smoke pass.
