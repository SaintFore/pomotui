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

## Cross-device synchronization

| Synchronization guarantee | Automated evidence |
|---|---|
| Two offline Devices retain disjoint Tasks, ended Sessions, Session Reviews, and totals and finally converge | `sync_process_convergence::two_timer_services_converge_and_recover_without_sharing_live_state`; service `two_databases_*` scenarios |
| Review Order and Action Chains converge independently of arrival order; late Session Reviews reproject history | service deterministic review-order and late-review scenarios; sync projection property tests |
| Chain Entry Title and Reflection edits converge and survive replay/restart | process convergence scenario; service chain-maintenance two-database scenarios |
| Reward Milestones, unlocks, and claims converge; claims survive reprojection | process convergence scenario; service reward synchronization and late-review scenarios |
| Current Sessions, Pending Reviews, and Focus Cycles stay per-Device | process convergence scenario; service `every_ended_session_shape_converges_while_pending_review_stays_local` and imported-cycle scenarios |
| Repeated import is idempotent and does not double totals | process convergence scenario; sync union idempotence and SQLite retry scenarios |
| An old-file overwrite is repaired from retained local records | process convergence scenario; worker external-replacement retry test |
| Restart preserves known Synchronization Records | process convergence scenario; SQLite two-replica restart scenarios |
| File deletion and rebuild recover everything known locally and state the unseen-remote limit | process convergence scenario; `sync_worker::asynchronous_rebuild_creates_a_missing_file_and_reports_its_limit` |
| A surviving replica later restores records absent from a rebuilt file | process convergence scenario; two-replica recovery scenarios |
| Invalid, unsupported, unreadable, or unwritable files do not mutate SQLite or block timer commands | process convergence scenario; sync document contract, filesystem adapter, and service failure-isolation tests |
| Mutable conflicts and deletions resolve deterministically | sync projection permutation/tie-break/tombstone tests; service two-database Task, chain, and reward scenarios |
| Sync commands have stable human and JSON behavior, including stale-service diagnosis | CLI sync parsing/rendering tests; protocol command tests; packaging stale-service tests |
| Sync disabled preserves all existing frontend and domain behavior | complete workspace suite; `tests/e2e.sh` |
| Provider-neutral setup, backup, path change, disable, recovery, and troubleshooting are documented | `docs/user-guide.md` synchronization guide |
| Startup, durable mutations, and the 30-second interval request automatic merges | `sync_worker::worker_synchronizes_once_when_it_starts`, `durable_mutation_through_protocol_handler_triggers_synchronization`, and `worker_synchronizes_again_on_the_fixed_interval` |
| Overlapping automatic requests coalesce and file I/O remains serialized | `sync_worker::overlapping_requests_coalesce_into_one_later_run` |
| Status exposes in-progress work, counts, warnings, and stage-specific failures | sync worker health-transition tests; CLI human/JSON sync-status tests |
| Imports and domain projections commit atomically | service SQLite rollback scenarios for Session Review and Shared Activity projection |
| Output replacement is validated and atomic; a write race converges on retry | platform sync-file adapter tests; `sync_worker::later_trigger_restores_retained_union_after_external_replacement_wins_a_write_race` |
| Imported Task deletion waits while a local Current Session references the Task | service `imported_task_deletion_waits_for_the_current_session_then_converges` |
| Implausible imported timestamps emit a non-blocking clock-skew warning | `sync_worker::worker_synchronizes_once_when_it_starts` |
| Sync configuration and path remain local; disable/path changes retain records | service `disabling_preserves_records_and_rebuild_recovers_a_valid_local_union`; CLI sync command tests |
| The complete file is size-, schema-, version-, integrity-, and semantic-validated before import | sync document contract tests; platform invalid-document replacement test |
| Pomotui exposes no provider dependency, Device registry, acknowledgement, or replication proof | protocol sync command surface tests; dependency manifests; user guide |

## Spec #1 conflict recovery, Reward Debt and Fresh Start

| Guarantee | Evidence |
|---|---|
| Actual offline divergent writes create Syncthing conflict copies; complete union survives cleanup | `python3 tests/syncthing_acceptance.py`; [run evidence](syncthing-acceptance.md) |
| Subset absorption, idle inode/mtime stability and stale-file union repair | real Syncthing runner; filesystem `sync_files` tests and worker idle/recovery scenarios |
| Invalid and contradictory same-ID copies remain diagnosable; unsafe cleanup retains evidence | real runner JSON/identity/symlink cases; deterministic cleanup/race tests |
| Claimed threshold-seven reward with one supporting success owes six | real runner late-review import; `reward_debt` service and sync allocation scenarios |
| Concurrent offline repayments count once; correction credits excess; history/config deletion preserve accounting | real runner with two real replica directories; deterministic debt permutations |
| Remote Fresh Start clears offline old work and reports notice; stale replay cannot resurrect history/debt | real runner; `fresh_start_process` and `fresh_start` fault tests |
| One copied main file carries activity and beginning to a fresh Device while settings remain local | real runner's third service; `fresh_start_process` |
| Supported released migrations preserve represented facts and IDs without fabricating legacy debt | `legacy_migration`, sync `document_contract`; actual archived baseline format-5 binary rejection in real runner |

Real transport commands, versions, barriers, results and limitations are recorded
in [Syncthing acceptance](syncthing-acceptance.md). Daily checks remain
`cargo test --workspace`; the real runner is opt-in because it requires Syncthing
and permission to open isolated localhost sockets.


## Focus Reservations in the TUI

| Guarantee | Automated evidence |
|---|---|
| Any eligible Pending type reserves current, selected, or no Task | TUI `reservation_can_use_current_selected_or_no_task_in_any_pending_kind` |
| Running, Paused and Pending Review cannot create reservations | TUI `reservation_creation_is_unavailable_during_running_paused_or_pending_session_review` |
| Reserved target/countdown render in both layouts and languages, early start works during rest | TUI `reservation_countdown_shows_reserved_target_and_can_start_during_rest` |
| Space starts the reserved Focus target, without choosing a different Task | TUI `reservation_space_starts_reserved_target_instead_of_selected_task` |
| Expiry defers during text entry; dismiss/reopen and rest remain available | TUI `expired_reservation_is_deferred_during_text_input_then_can_be_dismissed_and_reopened` |
| Cancellation allows optional Reflection and requires explicit destructive confirmation | TUI `reservation_cancellation_allows_empty_reflection_but_requires_explicit_confirmation` |
| Stale dialogs cannot fail a replacement reservation | TUI `stale_reservation_failure_dialog_cannot_judge_a_replacement_reservation` |
| Setting uses the authoritative service default, validates and submits persisted changes | TUI `reservation_default_editor_uses_service_snapshot_and_emits_persistent_setting`; service reservation config/restart tests |
| EN/zh-CN wide/narrow judgment and reward warnings remain visible | TUI `reservation_dialogs_show_judgment_and_reward_consequences_in_both_languages` |
| Archive distinguishes reservation failure without inventing duration | TUI `reservation_break_archive_labels_source_without_a_session_duration` |
