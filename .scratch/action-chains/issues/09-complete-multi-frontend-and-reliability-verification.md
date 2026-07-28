# 09 — Complete multi-frontend and reliability verification

**What to build:** Finish the Action Chain experience across every Timer Frontend and verify it under the operational conditions Pomotui promises: compact status, bounded history, narrow terminals, localization, protocol compatibility, restart recovery, retries, migrations, and regression safety.

**Blocked by:** 06 — Edit reflections while protecting chain history; 08 — Settle rewards after configuration changes and chain failure.

**Status:** ready-for-agent

- [x] Dashboard retains the Task list and presents a compact chain card with length, recent links, next reward, and Pending Review state.
- [x] Dashboard, Today, History, Chain, and Chain Archive navigation remains coherent and each view has a distinct purpose.
- [x] Chain and Chain Archive remain usable in supported narrow and wide terminal layouts.
- [x] English and Simplified Chinese presentation is complete, with `Reflection`/`复盘` localized and `Void` invariant.
- [x] Normal TUI views avoid internal IDs while detailed and JSON CLI output exposes stable chain, entry, review, Session, Task, milestone, unlock, and claim identities where relevant.
- [x] Service status remains compact; current-chain details, Ended Chains, and reward history use bounded queries.
- [x] The protocol version and compatibility errors accurately reflect the expanded command and response surface.
- [x] Mutation retries across disconnect and reconnect cannot duplicate reviews, entries, chain rollover, reward unlocks, edits, or claims.
- [x] Pending Review, current and Ended Chains, Void identity, edits, unlocks, claims, and unavailable rewards all survive Timer Service restart.
- [x] Existing databases migrate safely and repeat migration/startup without duplicating singleton or historical records.
- [x] Time passage, midnight, wall-clock changes, inactivity, shutdown, Break Sessions, Task completion, and claimed rewards never create or end an Action Chain.
- [x] Existing Focus Cycle, Session History, Task totals, stop/skip, reminders, degraded durability, and transport recovery behavior remains passing.
- [x] End-to-end tests cover deadline-to-review success, stopped-for-review success, restart with Pending Review, Focus rejection and Break allowance, failed review, archive inspection, reward unlock/claim, and failure settlement.
- [x] Formatting, linting, the full workspace test suite, and the repository end-to-end suite pass.
