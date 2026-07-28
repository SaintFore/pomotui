# 05 — Bound and diagnose Session Reminder retries

**What to build:** Retry transient Session Reminder failures predictably, stop
permanent failures from running forever, and let Timer Frontends show whether
effects are pending, retrying, delivered, or exhausted.

**Blocked by:** 04 — Persist and recover Session Reminder delivery.

**Status:** ready-for-agent

- [ ] Failed notification and sound effects retry with documented bounded
      backoff using a controllable clock.
- [ ] Both attempt and age limits prevent indefinite or unexpectedly late
      delivery.
- [ ] Exhausted effects remain available as terminal diagnostic records rather
      than disappearing.
- [ ] Protocol diagnostics expose aggregate pending, retrying, delivered, and
      exhausted counts without putting the complete journal in every Snapshot.
- [ ] CLI JSON and human-readable output distinguish each delivery state.
- [ ] Tests do not sleep in wall-clock time and cover retry scheduling,
      independent effects, exhaustion, restart, and duplicate-minimization
      boundaries.

