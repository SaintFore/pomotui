# 11 — Infer Void during successful Session Review

**What to build:** Make `S`/Enter the single successful-review action. If the Pending Review already has a Task, submit success immediately. If it has no Task, use the currently selected Task; when that selection is the system Void Task, ask for the required Chain Entry Title before submitting. Remove the separate `V` success path.

**Blocked by:** 10 — Show a Pending Review dialog.

**Status:** ready-for-agent

- [x] `S` and Enter submit a successful review immediately when the Session already has a Task.
- [x] For an unattributed Session, `S` and Enter assign the selected regular Task and submit success.
- [x] For an unattributed Session with Void selected, `S` and Enter ask for a Chain Entry Title and then submit success using Void.
- [x] The Pending Review dialog no longer advertises or handles a separate `V` success action.
- [x] English, Simplified Chinese, and narrow Ratatui rendering describe the single successful-review path.

## Comments

The user decides success or failure. Task attribution is a consequence of that judgment, so Void should be inferred from the selected Task instead of requiring a second success shortcut.

Resolved by making `S`/Enter the only successful-review action. Existing Task attribution is preserved, a selected regular Task is assigned, and selecting the system Void Task opens the required Chain Entry Title input.
