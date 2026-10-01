# 06: Sync rewards

**What to build:** Converge Reward Milestones, unlock state, and claims against the shared Action Chain while keeping real-world claims immutable when late Shared Activity changes historical projections.

**Blocked by:** 04 / Converge Session Reviews and Action Chains.

**Status:** resolved

- [ ] Reward Milestone creation, update, and deletion are versioned synchronization records with deterministic convergence.
- [ ] Devices with the same Shared Activity and Reward Milestone records derive the same current unlock eligibility.
- [ ] Duplicate unlock facts created by disconnected Devices for the same Reward Milestone and projected Action Chain collapse deterministically rather than producing duplicate rewards.
- [ ] Reward claims synchronize idempotently and cannot be offered for a second claim on another Device.
- [ ] A claimed reward remains a historical fact when a late Review changes chain boundaries or would otherwise invalidate its original unlock.
- [ ] An unlocked but unclaimed reward follows the latest projected Action Chain and may become unavailable after late Shared Activity.
- [ ] Unlock snapshots preserve reward name, threshold, and budget despite later configuration changes.
- [ ] Reward queries, CLI output, and existing TUI views display converged state without device-specific concepts.
- [ ] Two-database tests cover concurrent configuration, duplicate unlock, claim, late failure, deletion, restart, and reversed import order.
