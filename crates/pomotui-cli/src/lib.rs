#![allow(clippy::missing_errors_doc)]

use pomotui_protocol::{Command, PROTOCOL_VERSION, ProtocolError, Request, Response, SessionKind};

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
        ["stop"] => Command::Stop,
        ["skip"] => Command::Skip,
        ["task", "list"] => Command::TaskList,
        ["task", "create", title] => Command::TaskCreate { title: (*title).into() },
        ["task", "rename", id, title] => Command::TaskRename { id: parse_id(id)?, title: (*title).into() },
        ["task", "complete", id] => Command::TaskComplete { id: parse_id(id)? },
        ["task", "reopen", id] => Command::TaskReopen { id: parse_id(id)? },
        ["task", "delete", id] => Command::TaskDelete { id: parse_id(id)? },
        ["history"] => Command::History,
        ["summary"] => Command::Summary,
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
            "tooltip": format!("{:?} · round {}/{}", snapshot.kind, snapshot.completed_rounds, snapshot.rounds_per_cycle),
            "class": [snapshot.state.clone(), format!("{:?}", snapshot.kind).to_lowercase()],
            "percentage": percentage(snapshot.remaining_seconds, snapshot.planned_seconds),
        })).map_err(|error| error.to_string()),
        Response::Snapshot { snapshot } => Ok(format!(
            "{:?} {} · {} · round {}/{}",
            snapshot.kind,
            clock(snapshot.remaining_seconds),
            snapshot.state,
            snapshot.completed_rounds,
            snapshot.rounds_per_cycle
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
                        tasks: vec![],
                        today: pomotui_protocol::TodaySummary::default(),
                        recent_history: vec![],
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
