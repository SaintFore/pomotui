# 10 — Show a Pending Review dialog

**What to build:** When a Timer Frontend first observes a new Pending Review, the TUI opens a focused dialog that explains what is awaiting judgment and exposes the successful, failed, Task, and Void review paths. The user may dismiss it temporarily and reopen it with a documented shortcut.

**Blocked by:** 09 — Complete multi-frontend and reliability verification.

**Status:** ready-for-agent

- [x] A newly observed Pending Review automatically opens a dialog instead of appearing only as passive Chain-page text.
- [x] The dialog shows the reviewed Task or lack of attribution and the exact actual duration.
- [x] The dialog exposes clear keys for successful review, failed review, and Void assignment where applicable.
- [x] `Esc` dismisses the dialog without changing Pending Review and polling does not immediately reopen the same dismissed review.
- [x] Pressing `p` reopens the dialog while Pending Review still exists.
- [x] Submitting or clearing the review closes the dialog, while a later Pending Review opens it again.
- [x] The behavior works when Pending Review is present at TUI startup and when it arrives in a later service snapshot.
- [x] Wide, narrow, English, and Simplified Chinese Ratatui rendering is covered by regression tests.

## Comments

Reported from the Chain page: the service correctly blocked a new Focus Session, but the TUI displayed only `Pending Review` and the rejection message, leaving the review workflow undiscoverable.

Resolved by routing initial and polled service snapshots through a single TUI state transition. Each new Pending Review identity opens one dialog; dismissal is remembered for that identity, and `p` reopens it. Ratatui regression tests cover startup, polling, dismissal, replacement, narrow layout, and both interface languages.
