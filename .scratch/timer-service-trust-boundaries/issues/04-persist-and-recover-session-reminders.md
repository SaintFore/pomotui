# 04 — Persist and recover Session Reminder delivery

**What to build:** Preserve outstanding notification and sound delivery across
Timer Service crashes and restarts without ever advancing a completed Session
or Focus Cycle twice. Notification and sound are independent durable effects,
owned and dispatched by the Timer Service.

**Blocked by:** 03 — Expose and contain durable-state write failures.

**Status:** resolved

- [x] A deadline completion atomically persists Current Session progression,
      Session History, Focus Cycle advancement, the completion identity, and
      enabled Session Reminder effects.
- [x] Notification and sound use separate durable outbox records and one
      failing effect does not suppress the other.
- [x] The Timer Service attempts pending effects and acknowledges each only
      after its adapter reports success.
- [x] A restart recovers and dispatches committed but unacknowledged effects.
- [x] Unique completion identities prevent duplicate Session progression and
      duplicate outbox creation.
- [x] Tests cover failure before commit, after commit before dispatch, after
      external success before acknowledgement, and after acknowledgement.
- [x] The bounded at-least-once external-effect guarantee is documented without
      claiming impossible exactly-once desktop delivery.

## Comments

Added the version-two SQLite schema with a durable Session Reminder outbox and
non-destructive migration from version one. Completion identity, durable
service state, and enabled notification/sound rows commit in one transaction.
The Timer Service dispatches pending effects independently and acknowledges
only successful adapters; failed effects survive restart. Tests cover atomic
creation, duplicate completion identity, independent effect acknowledgement,
legacy migration, delivery failure, and restart recovery. The documented
contract remains once-only progression with bounded at-least-once external
effects. Workspace lint, tests, and process end-to-end smoke pass.
