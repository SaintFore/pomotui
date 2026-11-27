use std::collections::HashMap;

use chrono::{Days, Local, TimeZone};
use pomotui_domain::{SessionKind, SessionOutcome};
use pomotui_protocol::FocusActivityRange;
use serde::Serialize;

use crate::{ChainBreakSource, Service};

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Review<'a> {
    Pending,
    Successful {
        reflection: Option<&'a str>,
        chain_entry_title: Option<&'a str>,
    },
    Failed {
        reflection: &'a str,
        chain_entry_title: Option<&'a str>,
    },
}

impl Service {
    pub(super) fn focus_activity(&self, range: FocusActivityRange) -> serde_json::Value {
        let today = Local
            .timestamp_opt(self.wall, 0)
            .single()
            .unwrap_or_else(Local::now)
            .date_naive();
        let (first_offset, last_offset) = match range {
            FocusActivityRange::Today => (0, 0),
            FocusActivityRange::Yesterday => (1, 1),
            FocusActivityRange::LastSevenDays => (6, 0),
        };
        let from = today - Days::new(first_offset);
        let through = today - Days::new(last_offset);

        let mut sessions: Vec<_> = self
            .history
            .records()
            .iter()
            .filter_map(|record| {
                if record.kind != SessionKind::Focus || record.outcome == SessionOutcome::Skipped {
                    return None;
                }
                let ended_at = Local.timestamp_opt(record.ended_at, 0).single()?;
                (from..=through)
                    .contains(&ended_at.date_naive())
                    .then_some((record, ended_at))
            })
            .collect();
        sessions.sort_unstable_by_key(|(record, _)| (record.ended_at, record.id));

        let reviews = self.activity_reviews();
        let sessions: Vec<_> = sessions
            .into_iter()
            .map(|(record, ended_at)| {
                serde_json::json!({
                    "session_id": record.id,
                    "task_id": record.task_id.map(pomotui_domain::TaskId::get),
                    "task_title": record.task_title,
                    "ended_at": ended_at.to_rfc3339(),
                    "actual_seconds": record.actual_seconds,
                    "planned_seconds": record.planned_seconds,
                    "outcome": if record.outcome == SessionOutcome::Completed { "completed" } else { "stopped" },
                    "review": reviews.get(&record.id),
                })
            })
            .collect();

        serde_json::json!({
            "from_date": from.to_string(),
            "through_date": through.to_string(),
            "sessions": sessions,
        })
    }

    fn activity_reviews(&self) -> HashMap<u64, Review<'_>> {
        let links = self
            .chain_links
            .iter()
            .chain(self.ended_chains.iter().flat_map(|chain| &chain.links));
        let mut reviews: HashMap<_, _> = links
            .map(|link| {
                (
                    link.session_id,
                    Review::Successful {
                        reflection: link.reflection.as_deref(),
                        chain_entry_title: link.chain_entry_title.as_deref(),
                    },
                )
            })
            .collect();
        for chain in &self.ended_chains {
            let entry = &chain.chain_break;
            if entry.source == ChainBreakSource::SessionReview
                && let Some(session_id) = entry.session_id
            {
                reviews.insert(
                    session_id,
                    Review::Failed {
                        reflection: &entry.reflection,
                        chain_entry_title: entry.chain_entry_title.as_deref(),
                    },
                );
            }
        }
        if let Some(pending) = &self.pending_review {
            reviews.insert(pending.session_id, Review::Pending);
        }
        reviews
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainBreakState, ChainLinkState, EndedChainState, PendingReviewState};
    use pomotui_domain::{SessionRecord, TaskId};
    use serde_json::json;

    fn local_time(day: u32, hour: u32) -> i64 {
        Local
            .with_ymd_and_hms(2025, 1, day, hour, 0, 0)
            .earliest()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn activity_joins_retained_reviews_and_filters_by_local_end_date() {
        let mut service = Service::new();
        service.wall = local_time(8, 20);
        for (id, day, hour, kind, outcome) in [
            (4, 8, 13, SessionKind::Focus, SessionOutcome::Stopped),
            (3, 8, 12, SessionKind::Focus, SessionOutcome::Completed),
            (8, 8, 11, SessionKind::Focus, SessionOutcome::Skipped),
            (9, 8, 10, SessionKind::ShortBreak, SessionOutcome::Completed),
            (2, 7, 23, SessionKind::Focus, SessionOutcome::Completed),
            (1, 2, 12, SessionKind::Focus, SessionOutcome::Completed),
            (7, 1, 12, SessionKind::Focus, SessionOutcome::Completed),
        ] {
            service.history.push(SessionRecord {
                id,
                ended_at: local_time(day, hour),
                kind,
                outcome,
                planned_seconds: 1_500,
                actual_seconds: if id == 4 { 300 } else { 1_500 },
                task_id: Some(TaskId::new(11)),
                task_title: Some("Release notes".into()),
            });
        }
        service.ended_chains.push(EndedChainState {
            id: 1,
            links: vec![ChainLinkState {
                id: 7,
                session_id: 2,
                task_id: 11,
                task_title: "Release notes".into(),
                actual_seconds: 1_500,
                reflection: Some("Drafted installation section".into()),
                chain_entry_title: Some("Installation".into()),
            }],
            chain_break: ChainBreakState {
                id: 8,
                source: ChainBreakSource::SessionReview,
                daily_chain_goal: None,
                reservation_entity_id: None,
                session_id: Some(3),
                task_id: Some(11),
                task_title: Some("Release notes".into()),
                actual_seconds: Some(1_500),
                reflection: "Interrupted before verifying upgrades".into(),
                chain_entry_title: None,
            },
        });
        service.pending_review = Some(PendingReviewState {
            session_id: 4,
            actual_seconds: 300,
            task_id: Some(11),
            task_title: Some("Release notes".into()),
        });

        let today = service.focus_activity(FocusActivityRange::Today);
        assert_eq!(today["from_date"], "2025-01-08");
        assert_eq!(today["through_date"], "2025-01-08");
        let sessions = today["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["session_id"], 3);
        assert_eq!(sessions[0]["outcome"], "completed");
        assert_eq!(sessions[0]["review"]["status"], "failed");
        assert_eq!(sessions[1]["session_id"], 4);
        assert_eq!(sessions[1]["outcome"], "stopped");
        assert_eq!(sessions[1]["actual_seconds"], 300);
        assert_eq!(sessions[1]["review"], json!({"status": "pending"}));

        let yesterday = service.focus_activity(FocusActivityRange::Yesterday);
        assert_eq!(yesterday["from_date"], "2025-01-07");
        assert_eq!(yesterday["through_date"], "2025-01-07");
        assert_eq!(yesterday["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(
            yesterday["sessions"][0]["review"],
            json!({
                "status": "successful",
                "reflection": "Drafted installation section",
                "chain_entry_title": "Installation"
            })
        );

        let week = service.focus_activity(FocusActivityRange::LastSevenDays);
        assert_eq!(week["from_date"], "2025-01-02");
        let sessions = week["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 4);
        assert_eq!(sessions[0]["session_id"], 1);
        assert_eq!(sessions[0]["review"], serde_json::Value::Null);
    }
}
