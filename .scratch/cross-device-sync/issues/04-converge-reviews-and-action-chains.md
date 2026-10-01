# 04: Converge Session Reviews and Action Chains

**What to build:** Synchronize submitted Session Reviews and deterministically project them into the same current and Ended Chains on every installation, including when older offline Reviews arrive later.

**Blocked by:** 03 / Sync Session History and statistics.

**Status:** ready-for-agent

- [ ] Submitted successful and failed Session Reviews become immutable Shared Activity records linked to globally identified source Sessions.
- [ ] Pending Review remains local and is never exported before judgment is submitted.
- [ ] Review Order sorts by source Session end instant with stable Review identity as the tie-breaker and never depends on file arrival order.
- [ ] Equal record sets produce identical Chain Links, Chain Breaks, current Action Chain, and Ended Chains on every installation.
- [ ] A late successful or failed Review can revise past chain boundaries without rewriting the Review itself.
- [ ] Imported Reviews never change the importing Device's Current Session, Pending Review, or Focus Cycle.
- [ ] Implausible imported timestamps produce a non-blocking synchronization warning rather than silent rejection or timer failure.
- [ ] Retry, restart, reversed import order, equal timestamps, and late failure scenarios remain deterministic and idempotent.
- [ ] Service integration tests prove synchronization records and projected chain state commit atomically.

