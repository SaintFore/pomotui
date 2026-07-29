# 13 — Manage Reward Milestones in the TUI

**What to build:** Add a discoverable Reward Milestone manager to the Chain page. It must list the complete configured reward ladder and support creating, updating, and deleting milestones while preserving the existing unlock and claim semantics.

**Blocked by:** 12 — Browse and edit the current Action Chain.

**Status:** ready-for-agent

- [ ] Snapshots expose the complete ordered Reward Milestone configuration, including ID, threshold, name, and optional budget.
- [ ] The Chain page has a visible shortcut and summary leading to a Reward Milestone manager.
- [ ] The manager lists every milestone in threshold order and has a visible selection.
- [ ] The user can create a milestone with threshold, reward name, and optional budget.
- [ ] The user can update the selected milestone's threshold, reward name, and optional budget.
- [ ] The user can delete the selected milestone through explicit confirmation.
- [ ] Unlocked rewards can still be claimed from the Chain workflow.
- [ ] Empty, populated, narrow, English, and Simplified Chinese Ratatui rendering is covered.

## Comments

The Timer Service and CLI already support create/update/delete, but the TUI only has an undocumented create shortcut and current-chain unlock summaries. It does not expose the configured reward ladder, update, or deletion.
