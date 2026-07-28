# 08 — Settle rewards after configuration changes and chain failure

**What to build:** Complete Reward Milestone lifecycle rules: configuration changes can unlock rewards only for the active chain, configuration deletion cannot erase earned history, and failed review settles every unlocked reward while clearly warning the user what will become unavailable.

**Blocked by:** 05 — Review failure and archive the Action Chain; 07 — Configure, unlock, and claim Reward Milestones.

**Status:** ready-for-agent

- [ ] Adding a Reward Milestone at or below the current chain length immediately unlocks it once for the current Action Chain.
- [ ] Lowering an existing threshold to or below the current chain length immediately unlocks it once for the current Action Chain.
- [ ] Raising a threshold does not revoke an already unlocked reward.
- [ ] Configuration additions, edits, and deletions never recalculate or rewrite Ended Chain rewards.
- [ ] Deleting configuration removes only the future rule; unlock and claim snapshots remain durable.
- [ ] Failed-review confirmation lists every unlocked, unclaimed reward that will become unavailable.
- [ ] Canceling the confirmation leaves all reward states and Pending Review unchanged.
- [ ] Confirmed failure preserves claimed rewards in the Ended Chain.
- [ ] Confirmed failure marks unlocked but unclaimed rewards permanently unavailable.
- [ ] Reward settlement commits atomically with the Chain Break, Ended Chain, Pending Review clearing, and new current chain.
- [ ] Unavailable rewards cannot be claimed or restored.
- [ ] Retrying failure or configuration changes cannot duplicate unlocks or alter settled history.
- [ ] Service transaction, rollback, active-versus-ended-chain, protocol, CLI, Ratatui warning, retry, and restart tests verify every rule.

