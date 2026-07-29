# 17 — Delete whole Ended Chains and open scrollable archive details

**What to build:** Correct the Chain Archive interactions so `D` deletes the
selected Ended Chain—not the selected Task—through explicit confirmation, and
`Enter` opens a dedicated detail mode that can browse and edit arbitrarily long
Ended Chains.

**Blocked by:** 16 — Group TUI views into two Tab-switched areas.

**Status:** ready-for-agent

- [x] On the Chain Archive list, `D` opens an Ended Chain-specific destructive
  confirmation and never opens Task deletion.
- [x] Confirming deletion permanently removes the selected whole Ended Chain,
  including its Chain Links, Chain Break, and reward history.
- [x] Deleting an Ended Chain does not delete or alter its source Session
  History, Task focus totals, or any Task.
- [x] Canceling confirmation sends no mutation and preserves the Ended Chain.
- [x] Repeating the same deletion request is idempotent; deleting an unknown
  Ended Chain with a new request returns a stable error.
- [x] Ended Chain deletion survives Timer Service restart.
- [x] `Enter` on the selected Ended Chain opens its dedicated detail mode.
- [x] `Esc` returns from detail mode to the Chain Archive list without changing
  the selected Ended Chain.
- [x] `j`/`k` and arrow keys move a visible archived-entry cursor through every
  Chain Link and the terminal Chain Break, including chains longer than the
  terminal height.
- [x] Detail mode shows the selected entry's effective title, exact duration,
  and Reflection, plus the Ended Chain's reward history without internal IDs.
- [x] `E` edits the selected archived entry's Reflection; `T` edits the selected
  Void entry's Chain Entry Title.
- [x] `D` in detail mode cannot delete an individual Chain Link or Chain Break.
- [x] Empty, short, 100-link, wide, narrow, English, and Simplified Chinese
  archive states are covered.
- [x] Help and the user guide document list/detail navigation and whole-chain
  deletion.

## Test seams

Confirmed by the user before the first TDD red cycle:

1. The Timer Service `Handler` through protocol `Request`/`Response` is the
   public domain seam for whole Ended Chain deletion, cascading reward-history
   removal, Session History preservation, stable errors, and idempotency.
2. A durable Timer Service reopened from a real SQLite repository is the public
   persistence seam for deletion across restart.
3. `App::handle_key` is the public interaction seam for list deletion,
   confirmation/cancellation, Enter/Esc detail mode, cursor movement, and
   permitted edits.
4. Ratatui `render` through `TestBackend` is the public presentation seam for
   list/detail separation, long-chain browsing, localization, and responsive
   rendering.

Tests will not inspect private service collections, widget helpers, or SQLite
table layout.

## Comments

Reported from the Chain Archive after selecting an Ended Chain. `D` currently
falls through to the generic Task deletion branch and opens `ConfirmDelete`;
confirmation emits `TaskDelete`, so no Ended Chain is deleted and an unrelated
selected Task may be targeted. `Enter` similarly falls through to generic Task
selection instead of opening archive detail.

This ticket supersedes the earlier “Ended Chains cannot be deleted” decision in
issues 05 and 06. ADR-0007 records the new whole-chain deletion boundary:
individual entries remain non-deletable, while explicit whole-chain deletion
also removes archived reward history and preserves independent Session History.

Resolved on 2026-07-29. Protocol v4 adds idempotent whole Ended Chain deletion;
the Timer Service removes the chain and reward history in one durable mutation
while preserving Session History and Tasks. The Chain Archive now separates its
list from an Enter/Esc detail mode whose moving window can browse a 100-link
chain and edit only the selected entry's permitted text.
