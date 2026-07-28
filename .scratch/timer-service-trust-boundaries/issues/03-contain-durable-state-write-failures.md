# 03 — Expose and contain durable-state write failures

**What to build:** Make the Timer Service honest and safe when SQLite can no
longer accept state. Users can still inspect the Current Session and diagnostic
state, but further state-changing commands cannot widen the gap between memory
and the last durable commit.

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] A failed durable write places the Timer Service in an explicit degraded
      state with a safe diagnostic and last successful commit observation.
- [x] Subsequent state-changing commands are rejected before changing domain
      state while read-only requests remain available.
- [x] The first failed transition is never presented as unqualified durable
      success.
- [x] Timer Frontends can distinguish durable health through stable protocol
      data and errors.
- [x] Recovery is bounded and never silently replays rejected commands.
- [x] One service-level persistence seam supports both production SQLite and
      deterministic injected write failures.
- [x] Tests prove restart recovers the last committed state after failures at
      representative mutation and deadline boundaries.

## Comments

Implemented explicit healthy/degraded durable state in every Snapshot and a
stable durable-write protocol error. A failed write freezes automatic
progression, preserves read-only status and Task listing, and rejects later
mutations before domain state changes. A single Service repository port enables
deterministic write failure injection while production continues to use
SQLite. Tests prove the first volatile mutation is reported, later mutations
are contained, and restart recovers the last committed state. Workspace lint,
tests, and process end-to-end smoke pass.
