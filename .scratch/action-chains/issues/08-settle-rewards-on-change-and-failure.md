# 08 — Settle rewards after configuration changes and chain failure

**What to build:** Complete Reward Milestone lifecycle rules: configuration changes can unlock rewards only for the active chain, configuration deletion cannot erase earned history, and failed review settles every unlocked reward while clearly warning the user what will become unavailable.

**Blocked by:** 05 — Review failure and archive the Action Chain; 07 — Configure, unlock, and claim Reward Milestones.

**Status:** ready-for-agent

- [x] Adding a Reward Milestone at or below the current chain length immediately unlocks it once for the current Action Chain.
- [x] Lowering an existing threshold to or below the current chain length immediately unlocks it once for the current Action Chain.
- [x] Raising a threshold does not revoke an already unlocked reward.
- [x] Configuration additions, edits, and deletions never recalculate or rewrite Ended Chain rewards.
- [x] Deleting configuration removes only the future rule; unlock and claim snapshots remain durable.
- [x] Failed-review confirmation lists every unlocked, unclaimed reward that will become unavailable.
- [x] Canceling the confirmation leaves all reward states and Pending Review unchanged.
- [x] Confirmed failure preserves claimed rewards in the Ended Chain.
- [x] Confirmed failure marks unlocked but unclaimed rewards permanently unavailable.
- [x] Reward settlement commits atomically with the Chain Break, Ended Chain, Pending Review clearing, and new current chain.
- [x] Unavailable rewards cannot be claimed or restored.
- [x] Retrying failure or configuration changes cannot duplicate unlocks or alter settled history.
- [x] Service transaction, rollback, active-versus-ended-chain, protocol, CLI, Ratatui warning, retry, and restart tests verify every rule.

