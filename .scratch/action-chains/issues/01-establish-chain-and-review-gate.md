# 01 — Establish the Action Chain and Pending Review gate

**What to build:** Establish the smallest end-to-end Action Chain workflow: Pomotui always has one current empty Action Chain, a completed Focus Session creates a durable Pending Review, Break Sessions remain available, and another Focus Session is rejected until review. The compact state is observable from the Dashboard, CLI, and Timer Service protocol.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [x] Existing databases migrate to exactly one current Action Chain without changing existing Session History, Tasks, totals, or timer settings.
- [x] New databases start with exactly one current Action Chain whose length is zero.
- [x] A Focus Session reaching its deadline durably creates one Pending Review for that Session.
- [x] Pending Review survives Timer Frontend closure and Timer Service restart.
- [x] Starting and completing Break Sessions is permitted while Pending Review exists and does not clear or mutate it.
- [x] Starting another Focus Session while Pending Review exists returns a stable user-facing domain error.
- [x] Repeated completion or recovery processing cannot create duplicate Pending Reviews.
- [x] The authoritative service snapshot exposes a compact current-chain and Pending Review summary without embedding unbounded history.
- [x] The Dashboard displays current chain length and Pending Review state while retaining the existing Task list.
- [x] Human CLI status exposes the same compact state, and detailed or JSON output includes stable identities.
- [x] Domain scenarios, real-SQLite service tests, protocol tests, Ratatui tests, and restart coverage verify the delivered behavior.

