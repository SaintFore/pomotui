# 09: Verify convergence and document synchronization

**What to build:** Prove the complete feature across real processes and give users enough provider-neutral guidance to configure, recover, and safely reason about the single sync file.

**Blocked by:** 05 / Sync chain maintenance; 06 / Sync rewards; 07 / Automate, isolate, and rebuild synchronization; 08 / Migrate and export existing data safely.

**Status:** ready-for-agent

- [ ] Process-level tests run two Timer Service instances with isolated databases and one exchange file and prove final convergence of Tasks, Session History, totals, Reviews, Action Chains, chain edits, Reward Milestones, unlocks, and claims.
- [ ] End-to-end scenarios cover both services working offline, alternating file access, old-file overwrite, repeated import, service restart, file deletion, rebuild from one database, and later restoration of records retained by the other.
- [ ] End-to-end assertions prove Current Sessions, Pending Reviews, and Focus Cycles remain independent while imported Shared Activity converges.
- [ ] Failure scenarios prove invalid or inaccessible files do not corrupt either SQLite database or prevent ordinary timer commands.
- [ ] Existing CLI, TUI, tray, Waybar, reminder, recovery, task, history, chain, and reward regression suites remain green with synchronization disabled and enabled.
- [ ] The user guide explains generic single-file setup without prescribing a transport provider.
- [ ] Documentation explains automatic merging, deterministic conflict behavior, clock-skew warnings, rebuild semantics, database backups, and why `pomotui.sync` is not proof of replication or a replacement for backup.
- [ ] Documentation includes upgrade, path-change, disable, recovery, and troubleshooting procedures with human-readable and JSON CLI examples.
- [ ] The acceptance matrix links every synchronization guarantee to automated evidence.
- [ ] Formatting, linting, the complete workspace test suite, and the repository's end-to-end suite pass.
