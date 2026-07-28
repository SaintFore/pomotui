# 07 — Configure, unlock, and claim Reward Milestones

**What to build:** Let users define real-world Reward Milestones for Action Chain lengths, see them unlock exactly once as successful reviews grow the active chain, and claim them manually from the Chain page or CLI without any external fulfillment.

**Blocked by:** 02 — Review a Task Session as successful.

**Status:** ready-for-agent

- [x] A user can create, view, update, and delete Reward Milestone configuration with a positive threshold, name, and optional budget.
- [x] Successful review unlocks each eligible Reward Milestone exactly once for the current Action Chain.
- [x] Unlocking occurs in the same transaction as the Chain Link and snapshots milestone name, threshold, and optional budget.
- [x] Later configuration edits do not rewrite an existing unlock snapshot.
- [x] The Dashboard chain card shows the next configured reward without embedding unbounded reward history.
- [x] The Chain page shows configured, unlocked, and claimed reward states.
- [x] A user can explicitly claim an unlocked reward through the TUI or CLI.
- [x] Claiming records durable claim state and time but performs no purchase, payment, scheduling, or external action.
- [x] Claim retries and service restarts cannot create duplicate unlocks or duplicate claims.
- [x] A reward that has not unlocked cannot be claimed.
- [x] The protocol provides bounded reward configuration/history queries and idempotent mutation commands.
- [x] Real-SQLite transaction, threshold crossing, snapshot, retry, restart, protocol, CLI, Dashboard, and Ratatui tests verify the complete path.

