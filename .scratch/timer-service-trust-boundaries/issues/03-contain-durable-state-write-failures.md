# 03 — Expose and contain durable-state write failures

**What to build:** Make the Timer Service honest and safe when SQLite can no
longer accept state. Users can still inspect the Current Session and diagnostic
state, but further state-changing commands cannot widen the gap between memory
and the last durable commit.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] A failed durable write places the Timer Service in an explicit degraded
      state with a safe diagnostic and last successful commit observation.
- [ ] Subsequent state-changing commands are rejected before changing domain
      state while read-only requests remain available.
- [ ] The first failed transition is never presented as unqualified durable
      success.
- [ ] Timer Frontends can distinguish durable health through stable protocol
      data and errors.
- [ ] Recovery is bounded and never silently replays rejected commands.
- [ ] One service-level persistence seam supports both production SQLite and
      deterministic injected write failures.
- [ ] Tests prove restart recovers the last committed state after failures at
      representative mutation and deadline boundaries.

