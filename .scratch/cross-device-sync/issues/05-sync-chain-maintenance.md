# 05: Sync chain maintenance

**What to build:** Converge the user-editable and deletable parts of Action Chain history while preserving the immutable meaning of submitted Session Reviews.

**Blocked by:** 04 / Converge Session Reviews and Action Chains.

**Status:** resolved

- [x] Chain Entry Title and Reflection edits are versioned synchronization records and converge deterministically.
- [x] Later valid edits can correct wording selected from concurrent offline versions.
- [x] Synchronization cannot change a submitted judgment, source Session, Task attribution, entry kind, exact actual duration, or Task title snapshot.
- [x] Ended Chain deletion synchronizes as a durable tombstone and cannot be undone by stale file contents.
- [x] Deleting an Ended Chain continues to remove its chain entries and reward history while preserving Session History and Task totals according to existing domain rules.
- [x] A late Review is projected consistently with an already synchronized Ended Chain deletion rather than resurrecting deleted history unpredictably.
- [x] Current-chain and archive queries, CLI output, and existing TUI views reflect imported edits and deletions without exposing synchronization internals.
- [x] Two-database tests cover concurrent edits, deletion, stale replay, late Reviews, restart, and idempotent retries.

## Comments

Implemented append-only `chain_entry_version` and `ended_chain_deleted` records. Projection resolves text versions by mutation instant and record identity, permanently suppresses tombstoned Ended Chains, retains immutable Session Review facts, and preserves unrelated chain/reward identities. Sync-engine and two-database SQLite scenarios cover correction, reverse order, stale replay, late Reviews, restart, and retry convergence.
