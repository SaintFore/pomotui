# 02 — Review a Task Session as successful

**What to build:** Let the user review a Pending Review that already has a regular Task as successful. Submission atomically appends one Chain Link, clears Pending Review, and makes the reviewed work visible on a dedicated Chain page and through the CLI.

**Blocked by:** 01 — Establish the Action Chain and Pending Review gate.

**Status:** ready-for-agent

- [ ] A successful Session Review appends exactly one Chain Link to the current Action Chain.
- [ ] The Chain Link retains stable identity, source Session identity, Task identity, Task title snapshot, exact actual duration in seconds, and an optional Reflection.
- [ ] Submitting a successful review atomically clears Pending Review and increments current chain length.
- [ ] Review success is never inferred from elapsed duration, Task status, Completed Round status, or any other heuristic.
- [ ] The submitted judgment, Session identity, chain identity, Task attribution, entry kind, duration, and Task snapshot are immutable.
- [ ] Retrying the same review mutation cannot append a duplicate Chain Link or double-count Task time.
- [ ] The versioned protocol exposes successful review and bounded current-chain detail operations.
- [ ] The CLI can submit a successful review and render the resulting Chain Link.
- [ ] The TUI has a dedicated Chain page showing current length and Chain Links without displaying internal IDs in normal views.
- [ ] Chain displays show one actual duration value: exact minutes such as `50m`, or minutes and seconds such as `17m 32s`.
- [ ] The Dashboard current-chain summary updates immediately after review.
- [ ] Domain, service transaction, protocol, CLI, Ratatui, and retry tests verify the complete path.

