# 05 — Review failure and archive the Action Chain

**What to build:** Let the user explicitly judge a Pending Review as failed. After a warning and confirmation, Pomotui records a terminal non-counting Chain Break, turns the old chain into an immutable Ended Chain, immediately creates a new empty current chain, and exposes the result in a dedicated Chain Archive.

**Blocked by:** 03 — Support reviewable and unreviewed early stops; 04 — Attribute an unassigned review to a Task or Void.

**Status:** ready-for-agent

- [x] Failed Session Review requires a non-empty Reflection.
- [x] A failed review using the Void Task also requires a non-empty Chain Entry Title.
- [x] Before submission, the TUI shows an explicit confirmation containing the current chain length.
- [x] Canceling failure confirmation sends no mutation and leaves Pending Review unchanged.
- [x] Confirmed failure appends one terminal Chain Break that does not increase chain length.
- [x] The Chain Break retains source Session identity, Task identity and title snapshot, exact actual duration, required Reflection, and any required Void Chain Entry Title.
- [x] Recording the Chain Break, ending the old chain, clearing Pending Review, and creating exactly one new empty current chain is one atomic mutation.
- [x] Failure on a current chain with zero Chain Links produces a valid zero-link Ended Chain.
- [x] The submitted failure judgment and its Session, chain, Task attribution, entry kind, duration, and snapshot are immutable.
- [x] Ended Chains cannot be deleted, extended, reactivated, or merged.
- [x] A dedicated Chain Archive TUI page and bounded CLI/protocol queries expose Ended Chains separately from Session History.
- [x] Normal TUI archive views omit internal IDs; detailed and JSON CLI output retains stable IDs and exact seconds.
- [x] Retrying failure cannot create duplicate Chain Breaks, Ended Chains, or current chains.
- [x] Domain scenarios, real-SQLite rollback tests, protocol tests, CLI tests, Ratatui confirmation tests, zero-length tests, and restart tests verify the complete path.

