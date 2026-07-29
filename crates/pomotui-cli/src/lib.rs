#![allow(clippy::missing_errors_doc)]

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
        _ => return Err("usage: pomotui [--json] status|start focus [--task ID|--title TITLE]|start <short-break|long-break>|pause|resume|stop|skip|task ...|history|summary|waybar".into()),
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
            "{:?} {} · {} · round {}/{} · chain {}{}{}",
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
            reminder_delivery_label(&snapshot.reminder_delivery)
        )),
        Response::Data { value } => Ok(value.to_string()),
        Response::Accepted => Ok("accepted".into()),
        Response::Error { error } => Err(match error {
            ProtocolError::Rejected { message }
            | ProtocolError::Disconnected { message }
            | ProtocolError::Malformed { message } => message.clone(),
            other => format!("{other:?}"),
        }),
    }
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
            current_chain_rewards: vec![],
        };
        snapshot.pending_review = Some(pomotui_protocol::PendingReviewSummary {
            session_id: 4,
            actual_seconds: 1_500,
            task_id: Some(1),
            task_title: Some("Write tests".into()),
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
}
