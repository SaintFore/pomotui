# 03 — Support reviewable and unreviewed early stops

**What to build:** Give users explicit control over whether an early Focus Session stop should enter Session Review. The TUI offers a safe three-way choice, the CLI exposes explicit flags, and every path preserves the exact elapsed duration and existing Focus Cycle semantics.

**Blocked by:** 01 — Establish the Action Chain and Pending Review gate.

**Status:** ready-for-agent

- [x] Stopping a running Focus Session in the TUI offers “Stop and review,” “Stop without review,” and “Cancel.”
- [x] Cancel leaves the Current Session unchanged.
- [x] Stop and review writes the stopped Session to Session History and creates one durable Pending Review.
- [x] Stop without review writes the stopped Session to Session History but never creates Pending Review or changes the Action Chain.
- [x] The CLI supports explicit `stop --review` and `stop --no-review` behavior.
- [x] The legacy no-argument stop command remains compatible by behaving as stop without review.
- [x] A stopped Session retains its exact actual elapsed duration rather than its planned duration.
- [x] Neither stop path advances the Focus Cycle or adds a Completed Round.
- [x] A skipped Focus Session retains existing semantics and never creates Pending Review or changes the Action Chain.
- [x] Repeating or retrying a stop request cannot create duplicate Session History or Pending Review records.
- [x] Protocol, CLI parsing, Ratatui interaction, service transaction, and regression tests cover all three choices and both CLI flags.

