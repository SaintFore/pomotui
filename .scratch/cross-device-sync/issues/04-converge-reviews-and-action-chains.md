# 04: Converge Session Reviews and Action Chains

**What to build:** Synchronize submitted Session Reviews and deterministically project them into the same current and Ended Chains on every installation, including when older offline Reviews arrive later.

**Blocked by:** 03 / Sync Session History and statistics.

**Status:** resolved

- [x] Submitted successful and failed Session Reviews become immutable Shared Activity records linked to globally identified source Sessions.
- [x] Pending Review remains local and is never exported before judgment is submitted.
- [x] Review Order sorts by source Session end instant with stable Review identity as the tie-breaker and never depends on file arrival order.
- [x] Equal record sets produce identical Chain Links, Chain Breaks, current Action Chain, and Ended Chains on every installation.
- [x] A late successful or failed Review can revise past chain boundaries without rewriting the Review itself.
- [x] Imported Reviews never change the importing Device's Current Session, Pending Review, or Focus Cycle.
- [x] Implausible imported timestamps produce a non-blocking synchronization warning rather than silent rejection or timer failure.
- [x] Retry, restart, reversed import order, equal timestamps, and late failure scenarios remain deterministic and idempotent.
- [x] Service integration tests prove synchronization records and projected chain state commit atomically.

## Comments

Implemented immutable `session_reviewed` records, deterministic Review Order and full Action Chain reprojection. The Timer Service retains stable local entry mappings, backfills existing Reviews, attributes previously unattributed imported Sessions, warns on timestamps over one year from local time, and rolls record/history/chain changes back together when persistence fails.
