# 08: Migrate and export existing data safely

**What to build:** Let an existing Pomotui installation enable synchronization without losing or reinterpreting its current database, and export all previously durable Shared Activity into the new mergeable format.

**Blocked by:** 05 / Sync chain maintenance; 06 / Sync rewards.

**Status:** needs-triage

**Milestone:** Revisit after issue 09 freezes the stable data contract. Pre-release builds intentionally require a full local-data reset instead of migrating legacy state; see ADR 0009.

- [ ] Enabling synchronization on an existing database first creates a timestamped backup that can be restored independently of the sync file.
- [ ] The schema migration, global identity mapping, locally retained record creation, and initial projection changes run transactionally and roll back together on failure.
- [ ] Existing Tasks and their lifecycle state receive stable global mappings without changing local frontend-visible identities.
- [ ] Existing Focus and Break Session History, outcomes, durations, Task attribution, snapshots, and deletion state export exactly once.
- [ ] Existing submitted Reviews, current and Ended Chains, Chain Links, Chain Breaks, Chain Entry Titles, Reflections, and chain deletions export exactly once.
- [ ] Existing Reward Milestones, unlock snapshots, availability, claims, and deletion state export exactly once.
- [ ] Current Session, Pending Review, Focus Cycle, Session Durations, interface preferences, and transient frontend state remain local and are not written to the sync document.
- [ ] Migration preserves ordinary Tasks titled `Void` and the singleton system Void Task without conflating their identities.
- [ ] Retrying an interrupted or completed migration is idempotent and never duplicates exported domain facts.
- [ ] Independently migrated databases are treated as independent histories; coincidentally identical legacy records are not silently deduplicated.
- [ ] Representative production-shaped migration fixtures prove preservation, backup creation, rollback, restart safety, and successful import into a fresh second database.
