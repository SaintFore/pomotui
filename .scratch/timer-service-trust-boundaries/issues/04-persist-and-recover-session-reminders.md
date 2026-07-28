# 04 — Persist and recover Session Reminder delivery

**What to build:** Preserve outstanding notification and sound delivery across
Timer Service crashes and restarts without ever advancing a completed Session
or Focus Cycle twice. Notification and sound are independent durable effects,
owned and dispatched by the Timer Service.

**Blocked by:** 03 — Expose and contain durable-state write failures.

**Status:** ready-for-agent

- [ ] A deadline completion atomically persists Current Session progression,
      Session History, Focus Cycle advancement, the completion identity, and
      enabled Session Reminder effects.
- [ ] Notification and sound use separate durable outbox records and one
      failing effect does not suppress the other.
- [ ] The Timer Service attempts pending effects and acknowledges each only
      after its adapter reports success.
- [ ] A restart recovers and dispatches committed but unacknowledged effects.
- [ ] Unique completion identities prevent duplicate Session progression and
      duplicate outbox creation.
- [ ] Tests cover failure before commit, after commit before dispatch, after
      external success before acknowledgement, and after acknowledgement.
- [ ] The bounded at-least-once external-effect guarantee is documented without
      claiming impossible exactly-once desktop delivery.

