#![allow(clippy::missing_errors_doc)]

use std::fmt::Write as _;

use pomotui_protocol::{Command, PROTOCOL_VERSION, ProtocolError, Request, Response, SessionKind};

#[allow(clippy::too_many_lines)]
pub fn parse(args: &[String]) -> Result<(Command, bool, bool), String> {
    let json = args.iter().any(|arg| arg == "--json");
    let words: Vec<_> = args
        .iter()
        .filter(|arg| *arg != "--json")
        .map(String::as_str)
        .collect();
    let command = match words.as_slice() {
        ["fresh-start", "--confirm"] => Command::FreshStart { confirmed: true },
        ["fresh-start"] => return Err("Fresh Start requires --confirm".into()),
        ["status" | "waybar"] => Command::Status,
        ["start", "focus"] => Command::Start { kind: SessionKind::Focus, task_id: None },
        ["start", "focus", "--task", id] => Command::Start {
            kind: SessionKind::Focus,
            task_id: Some(parse_id(id)?),
        },
        ["start", "focus", "--title", title] => Command::StartTitle {
            title: (*title).into(),
        },
        ["start", "short-break"] => Command::Start { kind: SessionKind::ShortBreak, task_id: None },
        ["start", "long-break"] => Command::Start { kind: SessionKind::LongBreak, task_id: None },
        ["pause"] => Command::Pause,
        ["resume"] => Command::Resume,
        ["stop"] | ["stop", "--no-review"] => Command::Stop,
        ["stop", "--review"] => Command::StopReview,
        ["skip"] => Command::Skip,
        ["task", "list"] => Command::TaskList,
        ["task", "create", title] => Command::TaskCreate { title: (*title).into() },
        ["task", "rename", id, title] => Command::TaskRename { id: parse_id(id)?, title: (*title).into() },
        ["task", "complete", id] => Command::TaskComplete { id: parse_id(id)? },
        ["task", "reopen", id] => Command::TaskReopen { id: parse_id(id)? },
        ["task", "delete", id] => Command::TaskDelete { id: parse_id(id)? },
        ["history"] => Command::History,
        ["summary"] => Command::Summary,
        ["review", "success"] => Command::ReviewSuccess { reflection: None },
        ["review", "success", "--reflection", reflection] => Command::ReviewSuccess {
            reflection: Some((*reflection).into()),
        },
        ["review", "success", "--task", id] => Command::ReviewSuccessAssign {
            task_id: Some(parse_id(id)?),
            use_void: false,
            chain_entry_title: None,
            reflection: None,
        },
        ["review", "success", "--void", title] => Command::ReviewSuccessAssign {
            task_id: None,
            use_void: true,
            chain_entry_title: Some((*title).into()),
            reflection: None,
        },
        ["review", "success", "--void", title, "--reflection", reflection] => {
            Command::ReviewSuccessAssign {
                task_id: None,
                use_void: true,
                chain_entry_title: Some((*title).into()),
                reflection: Some((*reflection).into()),
            }
        }
        ["chain"] => Command::ActionChainCurrent,
        ["chain", "archive"] => Command::ActionChainArchive,
        ["chain", "edit", id, "--reflection", reflection] => Command::ChainEntryEdit {
            id: parse_id(id)?,
            reflection: Some((*reflection).into()),
            chain_entry_title: None,
        },
        ["chain", "edit", id, "--title", title] => Command::ChainEntryEdit {
            id: parse_id(id)?,
            reflection: None,
            chain_entry_title: Some((*title).into()),
        },
        ["chain", "edit", id, "--title", title, "--reflection", reflection] => {
            Command::ChainEntryEdit {
                id: parse_id(id)?,
                reflection: Some((*reflection).into()),
                chain_entry_title: Some((*title).into()),
            }
        }
        ["review", "failure", reflection] => Command::ReviewFailure {
            reflection: (*reflection).into(),
            task_id: None,
            use_void: false,
            chain_entry_title: None,
        },
        ["review", "failure", "--task", id, reflection] => Command::ReviewFailure {
            reflection: (*reflection).into(),
            task_id: Some(parse_id(id)?),
            use_void: false,
            chain_entry_title: None,
        },
        ["review", "failure", "--void", title, reflection] => Command::ReviewFailure {
            reflection: (*reflection).into(),
            task_id: None,
            use_void: true,
            chain_entry_title: Some((*title).into()),
        },
        ["reward", "list"] => Command::Rewards,
        ["reward", "create", threshold, name] => Command::RewardCreate {
            name: (*name).into(),
            threshold: parse_id(threshold)?,
            budget: None,
        },
        ["reward", "create", threshold, name, "--budget", budget] => Command::RewardCreate {
            name: (*name).into(),
            threshold: parse_id(threshold)?,
            budget: Some(parse_id(budget)?),
        },
        ["reward", "update", id, threshold, name] => Command::RewardUpdate {
            id: parse_id(id)?,
            name: (*name).into(),
            threshold: parse_id(threshold)?,
            budget: None,
        },
        ["reward", "delete", id] => Command::RewardDelete { id: parse_id(id)? },
        ["reward", "claim", id] => Command::RewardClaim {
            unlock_id: parse_id(id)?,
        },
        ["sync", "enable", path] => Command::SyncEnable { path: (*path).into() },
        ["sync", "disable"] => Command::SyncDisable,
        ["sync", "now"] => Command::SyncNow,
        ["sync", "rebuild"] => Command::SyncRebuild,
        ["sync", "status"] => Command::SyncStatus,
        _ => return Err("usage: pomotui [--json] status|fresh-start --confirm|start focus [--task ID|--title TITLE]|start <short-break|long-break>|pause|resume|stop|skip|task ...|history|summary|sync <enable PATH|disable|now|rebuild|status>|waybar".into()),
    };
    Ok((command, json, words == ["waybar"]))
}

fn parse_id(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid Task ID: {value}"))
}

#[must_use]
pub fn request(command: Command) -> Request {
    let key = command.mutates().then(|| {
        format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        )
    });
    Request {
        version: PROTOCOL_VERSION,
        idempotency_key: key,
        command,
    }
}

pub fn render(response: &Response, json: bool, waybar: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string(response).map_err(|error| error.to_string());
    }
    match response {
        Response::Snapshot { snapshot } if waybar => serde_json::to_string(&serde_json::json!({
            "text": format!("{} {}", snapshot.state, clock(snapshot.remaining_seconds)),
            "tooltip": format!("{:?} · round {}/{}{}", snapshot.kind, snapshot.completed_rounds, snapshot.rounds_per_cycle, reminder_delivery_label(&snapshot.reminder_delivery)),
            "class": [snapshot.state.clone(), format!("{:?}", snapshot.kind).to_lowercase()],
            "percentage": percentage(snapshot.remaining_seconds, snapshot.planned_seconds),
        })).map_err(|error| error.to_string()),
        Response::Snapshot { snapshot } => Ok(format!(
            "{:?} {} · {} · round {}/{} · chain {}{}{}{}",
            snapshot.kind,
            clock(snapshot.remaining_seconds),
            snapshot.state,
            snapshot.completed_rounds,
            snapshot.rounds_per_cycle,
            snapshot.action_chain.length,
            if snapshot.pending_review.is_some() {
                " · pending review"
            } else {
                ""
            },
            reminder_delivery_label(&snapshot.reminder_delivery),
            snapshot.reward_debt.iter().fold(String::new(), |mut text, debt| {
                let _ = write!(text, " · reward debt {} (repaid {}, credit {})", debt.outstanding, debt.repaid, debt.excess_credit);
                text
            })
        )),
        Response::Data { value }
            if value.get("format_version").is_some()
                && value.get("local_record_count").is_some() => Ok(format!(
            "sync {}{} · {} · capabilities {} · path {} · format v{} · local records {} · file records {}{}{}",
            if value["enabled"].as_bool().unwrap_or(false) { "enabled" } else { "disabled" },
            if value["in_progress"].as_bool().unwrap_or(false) { " (in progress)" } else { "" },
            value["stability"].as_str().unwrap_or("unknown stability"),
            value["capabilities"].as_array().map_or_else(|| "none".into(), |items| items.iter().filter_map(serde_json::Value::as_str).collect::<Vec<_>>().join(",")),
            value["path"].as_str().unwrap_or("not configured"),
            value["format_version"],
            value["local_record_count"],
            value["file_record_count"].as_u64().map_or_else(|| "unknown".into(), |count| count.to_string()),
            value["last_error"].as_str().map_or_else(String::new, |error| format!(" · {} error: {error}", value["last_error_stage"].as_str().unwrap_or("sync"))),
            value["warning"].as_str().map_or_else(String::new, |warning| format!(" · warning: {warning}")),
        )),
        Response::Data { value } => Ok(value.to_string()),
        Response::Accepted => Ok("accepted".into()),
        Response::Error { error } => Err(match error {
            ProtocolError::Malformed { message } if stale_sync_service(message) => format!(
                "the installed CLI reached an older Timer Service that does not support synchronization; restart the Timer Service and retry ({message})"
            ),
            ProtocolError::Rejected { message }
            | ProtocolError::Disconnected { message }
            | ProtocolError::Malformed { message } => message.clone(),
            other => format!("{other:?}"),
        }),
    }
}

fn stale_sync_service(message: &str) -> bool {
    message.contains("unknown variant")
        && [
            "SyncEnable",
            "SyncDisable",
            "SyncNow",
            "SyncRebuild",
            "SyncStatus",
            "sync_enable",
            "sync_disable",
            "sync_now",
            "sync_rebuild",
            "sync_status",
        ]
        .iter()
        .any(|operation| message.contains(operation))
}

fn reminder_delivery_label(delivery: &pomotui_protocol::ReminderDelivery) -> String {
    if delivery.exhausted > 0 {
        format!(" · reminders exhausted: {}", delivery.exhausted)
    } else if delivery.retrying > 0 {
        format!(" · reminders retrying: {}", delivery.retrying)
    } else if delivery.pending > 0 {
        format!(" · reminders pending: {}", delivery.pending)
    } else {
        String::new()
    }
}

fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResetOutcome {
    pub backup: Option<std::path::PathBuf>,
}

pub fn reset_all_data(
    database: &std::path::Path,
    socket: &std::path::Path,
) -> Result<ResetOutcome, String> {
    if std::os::unix::net::UnixStream::connect(socket).is_ok() {
        return Err("Timer Service is running; stop it before resetting all local data".into());
    }
    let parent = database
        .parent()
        .ok_or_else(|| "database path has no parent directory".to_owned())?;
    let backup = if database.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("cannot timestamp database backup: {error}"))?
            .as_nanos();
        let file_name = database
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| "database path has no valid file name".to_owned())?;
        let backup = parent.join(format!("{file_name}.backup-{stamp}"));
        std::fs::copy(database, &backup)
            .map_err(|error| format!("cannot create database backup: {error}"))?;
        std::fs::File::open(&backup)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("cannot flush database backup: {error}"))?;
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("cannot flush database backup directory: {error}"))?;
        Some(backup)
    } else {
        None
    };

    for path in [
        database.to_path_buf(),
        std::path::PathBuf::from(format!("{}-wal", database.display())),
        std::path::PathBuf::from(format!("{}-shm", database.display())),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot remove {}: {error}", path.display())),
        }
    }
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot flush reset database directory: {error}"))?;
    Ok(ResetOutcome { backup })
}

fn percentage(remaining: u64, planned: u64) -> u64 {
    if planned == 0 {
        0
    } else {
        remaining
            .saturating_mul(100)
            .checked_div(planned)
            .unwrap_or(0)
            .min(100)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fresh_start_requires_explicit_confirmation() {
        let missing = super::parse(&["fresh-start".into()]).expect_err("confirmation required");
        assert!(missing.contains("--confirm"));
        let (command, json, _) =
            super::parse(&["fresh-start".into(), "--confirm".into(), "--json".into()])
                .expect("confirmed fresh start");
        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            serde_json::json!({"command":"fresh_start", "confirmed":true})
        );
        assert!(command.mutates());
        assert!(json);
    }

    use super::*;
    use pomotui_protocol::Snapshot;

    #[test]
    fn waybar_output_has_stable_fields() {
        let value: serde_json::Value = serde_json::from_str(
            &render(
                &Response::Snapshot {
                    snapshot: Snapshot {
                        state: "paused".into(),
                        kind: SessionKind::Focus,
                        remaining_seconds: 90,
                        planned_seconds: 100,
                        current_task: None,
                        current_task_id: None,
                        completed_rounds: 2,
                        rounds_per_cycle: 4,
                        next_kind: None,
                        durable_health: pomotui_protocol::DurableHealth {
                            state: pomotui_protocol::DurableHealthState::Healthy,
                            last_successful_commit: None,
                            error: None,
                        },
                        reminder_delivery: pomotui_protocol::ReminderDelivery::default(),
                        tasks: vec![],
                        today: Box::new(pomotui_protocol::TodaySummary::default()),
                        recent_history: vec![],
                        action_chain: pomotui_protocol::ActionChainSummary::default(),
                        pending_review: None,
                        recent_chain_links: vec![],
                        recent_ended_chains: vec![],
                        next_reward: None,
                        reward_milestones: vec![],
                        reward_debt: vec![],
                        current_chain_rewards: vec![],
                    },
                },
                false,
                true,
            )
            .expect("render"),
        )
        .expect("json");
        assert_eq!(value["text"], "paused 01:30");
        assert_eq!(value["class"], serde_json::json!(["paused", "focus"]));
    }

    #[test]
    fn mutation_request_has_identity_but_status_does_not() {
        assert!(request(Command::Pause).idempotency_key.is_some());
        assert!(request(Command::Status).idempotency_key.is_none());
        assert!(request(Command::SyncStatus).idempotency_key.is_none());
    }

    #[test]
    fn sync_commands_are_available_to_human_and_json_frontends() {
        let (command, json, _) =
            parse(&["sync".into(), "enable".into(), "/tmp/pomotui.sync".into()])
                .expect("sync enable");
        assert_eq!(
            command,
            Command::SyncEnable {
                path: "/tmp/pomotui.sync".into()
            }
        );
        assert!(!json);
        assert_eq!(
            parse(&["sync".into(), "disable".into()])
                .expect("sync disable")
                .0,
            Command::SyncDisable
        );
        assert_eq!(
            parse(&["sync".into(), "rebuild".into()])
                .expect("sync rebuild")
                .0,
            Command::SyncRebuild
        );
        assert_eq!(
            parse(&[
                "sync".into(),
                "enable".into(),
                "/tmp/path with spaces/pomotui.sync".into(),
            ])
            .expect("sync path with spaces")
            .0,
            Command::SyncEnable {
                path: "/tmp/path with spaces/pomotui.sync".into()
            }
        );
        let (_, json, _) =
            parse(&["--json".into(), "sync".into(), "status".into()]).expect("JSON sync status");
        assert!(json);
    }

    #[test]
    fn usage_error_lists_every_sync_operation() {
        let error = parse(&["sync".into(), "unknown".into()]).expect_err("unknown operation");

        for operation in ["enable", "disable", "now", "rebuild", "status"] {
            assert!(
                error.contains(operation),
                "usage must mention sync {operation}: {error}"
            );
        }
    }

    #[test]
    fn sync_health_is_visible_in_human_and_json_output() {
        let response = Response::Data {
            value: serde_json::json!({
                "stability": "experimental",
                "capabilities": ["task_lifecycle"],
                "enabled": true,
                "path": "/tmp/path with spaces/pomotui.sync",
                "format_version": 2,
                "in_progress": true,
                "last_attempt": 10,
                "last_success": 9,
                "last_error": "changed repeatedly",
                "last_error_stage": "compare",
                "warning": "unseen remote records cannot be recovered",
                "local_record_count": 3,
                "file_record_count": 2
            }),
        };

        let human = render(&response, false, false).expect("human status");
        assert!(human.contains("in progress"));
        assert!(human.contains("compare error: changed repeatedly"));
        assert!(human.contains("unseen remote records"));
        let json: serde_json::Value =
            serde_json::from_str(&render(&response, true, false).expect("JSON status"))
                .expect("JSON response");
        assert_eq!(json["value"]["last_error_stage"], "compare");
        assert_eq!(json["value"]["file_record_count"], 2);
    }

    #[test]
    fn reminder_delivery_state_is_visible_in_human_and_json_status() {
        let response = Response::Snapshot {
            snapshot: Snapshot {
                state: "pending".into(),
                kind: SessionKind::Focus,
                remaining_seconds: 1_500,
                planned_seconds: 1_500,
                current_task: None,
                current_task_id: None,
                completed_rounds: 0,
                rounds_per_cycle: 4,
                next_kind: None,
                durable_health: pomotui_protocol::DurableHealth {
                    state: pomotui_protocol::DurableHealthState::Healthy,
                    last_successful_commit: None,
                    error: None,
                },
                reminder_delivery: pomotui_protocol::ReminderDelivery {
                    retrying: 2,
                    ..pomotui_protocol::ReminderDelivery::default()
                },
                tasks: vec![],
                today: Box::new(pomotui_protocol::TodaySummary::default()),
                recent_history: vec![],
                action_chain: pomotui_protocol::ActionChainSummary::default(),
                pending_review: None,
                recent_chain_links: vec![],
                recent_ended_chains: vec![],
                next_reward: None,
                reward_milestones: vec![],
                reward_debt: vec![],
                current_chain_rewards: vec![],
            },
        };

        assert!(
            render(&response, false, false)
                .expect("human status")
                .contains("reminders retrying: 2")
        );
        let json: serde_json::Value =
            serde_json::from_str(&render(&response, true, false).expect("json")).expect("value");
        assert_eq!(json["snapshot"]["reminder_delivery"]["retrying"], 2);
    }

    #[test]
    fn human_status_shows_chain_length_and_pending_review() {
        let mut snapshot = Snapshot {
            state: "pending".into(),
            kind: SessionKind::ShortBreak,
            remaining_seconds: 300,
            planned_seconds: 300,
            current_task: Some("Write tests".into()),
            current_task_id: Some(1),
            completed_rounds: 1,
            rounds_per_cycle: 4,
            next_kind: Some(SessionKind::Focus),
            durable_health: pomotui_protocol::DurableHealth {
                state: pomotui_protocol::DurableHealthState::Healthy,
                last_successful_commit: None,
                error: None,
            },
            reminder_delivery: pomotui_protocol::ReminderDelivery::default(),
            tasks: vec![],
            today: Box::new(pomotui_protocol::TodaySummary::default()),
            recent_history: vec![],
            action_chain: pomotui_protocol::ActionChainSummary { id: 7, length: 12 },
            pending_review: None,
            recent_chain_links: vec![],
            recent_ended_chains: vec![],
            next_reward: None,
            reward_milestones: vec![],
            reward_debt: vec![],
            current_chain_rewards: vec![],
        };
        snapshot.pending_review = Some(pomotui_protocol::PendingReviewSummary {
            session_id: 4,
            actual_seconds: 1_500,
            task_id: Some(1),
            task_title: Some("Write tests".into()),
            is_void: false,
        });

        let rendered = render(&Response::Snapshot { snapshot }, false, false).expect("status");
        assert!(rendered.contains("chain 12"));
        assert!(rendered.contains("pending review"));
    }

    #[test]
    fn disconnected_error_is_actionable_and_json_stays_structured() {
        let response = Response::Error {
            error: ProtocolError::Disconnected {
                message: "socket unavailable".into(),
            },
        };
        assert_eq!(
            render(&response, false, false),
            Err("socket unavailable".into())
        );
        let json = render(&response, true, false).expect("json");
        assert!(json.contains("\"code\":\"disconnected\""));
    }

    #[test]
    fn unknown_sync_operation_identifies_a_stale_timer_service() {
        let response = Response::Error {
            error: ProtocolError::Malformed {
                message: "unknown variant `SyncNow`, expected `Status`".into(),
            },
        };

        let error = render(&response, false, false).expect_err("request must fail");

        assert!(error.contains("older Timer Service"));
        assert!(error.contains("restart"));
    }

    #[test]
    fn unrelated_malformed_requests_do_not_claim_the_service_is_stale() {
        let response = Response::Error {
            error: ProtocolError::Malformed {
                message: "request frame is missing a terminator".into(),
            },
        };

        assert_eq!(
            render(&response, false, false),
            Err("request frame is missing a terminator".into())
        );
    }

    #[test]
    fn reset_refuses_a_reachable_timer_service() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-reset-live-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temporary directory");
        let database = root.join("pomotui.sqlite3");
        std::fs::write(&database, "database").expect("database");
        let socket = root.join("pomotui.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("listener");

        let error = reset_all_data(&database, &socket).expect_err("live service must block reset");

        assert!(error.contains("Timer Service is running"));
        assert_eq!(
            std::fs::read_to_string(&database).expect("database remains"),
            "database"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reset_backs_up_database_and_only_removes_sqlite_files() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-reset-files-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temporary directory");
        let database = root.join("pomotui.sqlite3");
        std::fs::write(&database, "durable state").expect("database");
        std::fs::write(root.join("pomotui.sqlite3-wal"), "wal").expect("wal");
        std::fs::write(root.join("pomotui.sqlite3-shm"), "shm").expect("shm");
        let config = root.join("config.toml");
        let sync_file = root.join("chosen.sync");
        std::fs::write(&config, "theme = 'test'").expect("config");
        std::fs::write(&sync_file, "sync").expect("sync file");

        let outcome = reset_all_data(&database, &root.join("missing.sock")).expect("safe reset");

        let backup = outcome.backup.expect("backup path");
        assert_eq!(
            std::fs::read_to_string(backup).expect("backup"),
            "durable state"
        );
        assert!(!database.exists());
        assert!(!root.join("pomotui.sqlite3-wal").exists());
        assert!(!root.join("pomotui.sqlite3-shm").exists());
        assert!(config.exists());
        assert!(sync_file.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
