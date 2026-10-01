# 05: Sync chain maintenance

**What to build:** Converge the user-editable and deletable parts of Action Chain history while preserving the immutable meaning of submitted Session Reviews.

**Blocked by:** 04 / Converge Session Reviews and Action Chains.

**Status:** ready-for-agent

- [ ] Chain Entry Title and Reflection edits are versioned synchronization records and converge deterministically.
- [ ] Later valid edits can correct wording selected from concurrent offline versions.
- [ ] Synchronization cannot change a submitted judgment, source Session, Task attribution, entry kind, exact actual duration, or Task title snapshot.
- [ ] Ended Chain deletion synchronizes as a durable tombstone and cannot be undone by stale file contents.
- [ ] Deleting an Ended Chain continues to remove its chain entries and reward history while preserving Session History and Task totals according to existing domain rules.
- [ ] A late Review is projected consistently with an already synchronized Ended Chain deletion rather than resurrecting deleted history unpredictably.
- [ ] Current-chain and archive queries, CLI output, and existing TUI views reflect imported edits and deletions without exposing synchronization internals.
- [ ] Two-database tests cover concurrent edits, deletion, stale replay, late Reviews, restart, and idempotent retries.

