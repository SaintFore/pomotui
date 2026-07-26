# Pomotui v1 acceptance matrix

| Spec criterion | Evidence |
|---|---|
| Shared Current Session | protocol simultaneous-client and polling-reconnect tests; process E2E |
| Frontend-independent progression | background deadline-ticker completion test; service/socket packaging |
| Completed/stopped/skipped cycle rules | `session_scenarios` |
| Actual duration and Task attribution | `session_scenarios`, `task_history_scenarios` |
| Recovery and once-only reminders | same/new-boot recovery, deadline/restart, atomic reminder-claim, and effect-isolation tests |
| Task ambiguity/deletion/snapshots | `task_history_scenarios` |
| Wide/narrow state/theme rendering | `pomotui-tui` TestBackend state matrix |
| Keyboard/palette/settings/mouse | TUI navigation, responsive settings, semantic-color, and command-action tests |
| CLI JSON and Waybar | CLI schema/disconnect tests plus restart E2E |
| External effects and animation isolation | reminder and animation fallback tests |
| Linux install/socket/TUI/CLI/Waybar | `systemd-socket-activate` E2E, user units, installer preservation checks, user guide, release build |
