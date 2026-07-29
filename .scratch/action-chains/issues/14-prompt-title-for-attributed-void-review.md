# 14 — Prompt for a title when Pending Review is already attributed to Void

**What to build:** Treat an existing system Void attribution the same as selecting Void during an unattributed successful Session Review. Pressing `S`/Enter must request the required Chain Entry Title before submitting.

**Blocked by:** 11 — Infer Void during successful Session Review.

**Status:** ready-for-agent

- [x] A Pending Review whose Task identity is the system Void Task opens Chain Entry Title input on `S`/Enter.
- [x] It does not emit `ReviewSuccess` without the required title.
- [x] Submitting the title records success against the existing Void attribution.
- [x] A Pending Review attributed to a regular Task still submits success immediately.
- [x] A Ratatui/App regression test covers the exact already-attributed Void scenario.

## Comments

Reported from the Dashboard after a Focus Session was started with Void selected. The review dialog displayed Void, but `S` submitted `ReviewSuccess` without a Chain Entry Title and the Timer Service rejected it.

Root cause: the TUI treated every non-null Pending Review `task_id` as a regular Task. Resolved by having the Timer Service expose the authoritative `is_void` fact and routing both the dialog and Chain-page success actions through one decision function.
