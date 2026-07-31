use pomotui_protocol::{SessionKind, Snapshot};

/// Formats the menu bar display text from a service snapshot.
///
/// Shows an icon prefix and remaining time in `MM:SS` format.
/// Returns `None` when the service is unreachable.
#[must_use]
pub fn format_tray_text(snapshot: &Snapshot) -> String {
    let icon = match snapshot.state.as_str() {
        "running" => "▶",
        "paused" => "⏸",
        _ => "⏹",
    };
    let minutes = snapshot.remaining_seconds / 60;
    let seconds = snapshot.remaining_seconds % 60;
    format!("{icon} {minutes:02}:{seconds:02}")
}

/// Returns the status line shown at the top of the context menu.
#[must_use]
pub fn format_status_line(snapshot: &Snapshot) -> String {
    let state_label = match snapshot.state.as_str() {
        "running" => "Running",
        "paused" => "Paused",
        "pending" => "Pending",
        _ => &snapshot.state,
    };
    let kind_label = match snapshot.kind {
        SessionKind::Focus => "Focus",
        SessionKind::ShortBreak => "Short Break",
        SessionKind::LongBreak => "Long Break",
    };
    let round = format!(
        "{}/{}",
        snapshot.completed_rounds, snapshot.rounds_per_cycle
    );
    let task = snapshot.current_task.as_deref().unwrap_or("No task");
    format!("{state_label} {kind_label} · round {round} · {task}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pomotui_protocol::{DurableHealth, ReminderDelivery};

    use pomotui_protocol::DurableHealthState;

    fn make_snapshot(state: &str, kind: SessionKind, remaining: u64) -> Snapshot {
        Snapshot {
            state: state.to_owned(),
            kind,
            remaining_seconds: remaining,
            planned_seconds: 1500,
            current_task: Some("Write code".to_owned()),
            current_task_id: Some(1),
            completed_rounds: 2,
            rounds_per_cycle: 4,
            next_kind: None,
            durable_health: DurableHealth {
                state: DurableHealthState::Healthy,
                last_successful_commit: None,
                error: None,
            },
            reminder_delivery: ReminderDelivery {
                pending: 0,
                retrying: 0,
                delivered: 0,
                exhausted: 0,
            },
            tasks: vec![],
            today: Box::default(),
            recent_history: vec![],
            action_chain: pomotui_protocol::ActionChainSummary::default(),
            pending_review: None,
            recent_chain_links: vec![],
            recent_ended_chains: vec![],
            next_reward: None,
            reward_milestones: vec![],
            current_chain_rewards: vec![],
        }
    }

    #[test]
    fn running_focus_shows_play_icon_with_time() {
        let snap = make_snapshot("running", SessionKind::Focus, 1500);
        assert_eq!(format_tray_text(&snap), "▶ 25:00");
    }

    #[test]
    fn paused_shows_pause_icon() {
        let snap = make_snapshot("paused", SessionKind::Focus, 900);
        assert_eq!(format_tray_text(&snap), "⏸ 15:00");
    }

    #[test]
    fn pending_shows_stop_icon_with_zero() {
        let snap = make_snapshot("pending", SessionKind::Focus, 0);
        assert_eq!(format_tray_text(&snap), "⏹ 00:00");
    }

    #[test]
    fn remaining_seconds_padded() {
        let snap = make_snapshot("running", SessionKind::Focus, 65);
        assert_eq!(format_tray_text(&snap), "▶ 01:05");
    }

    #[test]
    fn status_line_includes_kind_and_round() {
        let snap = make_snapshot("running", SessionKind::Focus, 1500);
        assert_eq!(
            format_status_line(&snap),
            "Running Focus · round 2/4 · Write code"
        );
    }

    #[test]
    fn status_line_shows_no_task_when_absent() {
        let mut snap = make_snapshot("pending", SessionKind::ShortBreak, 0);
        snap.current_task = None;
        assert_eq!(
            format_status_line(&snap),
            "Pending Short Break · round 2/4 · No task"
        );
    }
}
