# 03: Sync Session History and statistics

**What to build:** Converge ended Focus and Break Sessions so that every installation reports the same cross-device Session History and totals without allowing imported activity to alter its local Focus Cycle.

**Blocked by:** 07 / Automate, isolate, and rebuild synchronization.

**Status:** ready-for-agent

- [ ] Completed, Stopped, and Skipped Focus Sessions and ended Break Sessions become globally identified Shared Activity records.
- [ ] Imported Session History preserves planned duration, exact actual duration, outcome, source Task identity, and Task title snapshot.
- [ ] Attributed Sessions resolve through synchronized global Task identity even when each database uses different local numeric identifiers.
- [ ] Total focus time, daily summaries, per-Task totals, and Completed Round statistics converge without merging mutable counters or double-counting repeated imports.
- [ ] Imported Completed Rounds do not advance or reset the importing Device's Focus Cycle and do not replace its Current Session or Pending Session.
- [ ] An ended Focus Session can synchronize while its Pending Review remains local to the source Device.
- [ ] Session History deletion is represented by a tombstone and preserves the existing rule that related Action Chain history is not deleted.
- [ ] Two-database scenarios cover all Session outcomes, Break Sessions, attributed and unattributed history, deletion, retry, and local Focus Cycle isolation.
- [ ] Existing history, summary, TUI, CLI, tray, and Waybar behavior remains compatible with imported records.
