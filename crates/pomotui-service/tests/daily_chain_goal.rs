mod support;

use pomotui_domain::SessionDurations;
use pomotui_protocol::{
    Command, DailyChainGoalSettings, Handler, PROTOCOL_VERSION, Request, Response, Snapshot,
};
use pomotui_service::Service;
use pomotui_sync::RecordId;
use support::ManualClock;

fn request(service: &mut Service, command: Command) -> Response {
    service.handle(Request {
        version: PROTOCOL_VERSION,
        idempotency_key: command
            .mutates()
            .then(|| RecordId::random().as_str().into()),
        command,
    })
}

fn snapshot(service: &mut Service, command: Command) -> Snapshot {
    let response = request(service, command);
    let Response::Snapshot { snapshot } = response else {
        panic!("expected snapshot: {response:?}");
    };
    snapshot
}

fn goal(quota: u64, start: Option<&str>, end: Option<&str>, zone: &str) -> Command {
    Command::ConfigureDailyChainGoal {
        settings: Some(DailyChainGoalSettings {
            quota,
            start_date: start.map(str::to_owned),
            end_date: end.map(str::to_owned),
            time_zone: zone.into(),
        }),
    }
}

fn at(date_time: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(date_time)
        .unwrap()
        .timestamp()
}

#[test]
fn a_goal_defaults_to_tomorrow_validates_and_keeps_todays_commitment() {
    let clock = ManualClock::new(at("2026-11-20T15:00:00Z"));
    let mut service = Service::new_with_clock(clock.clone());
    let off = snapshot(&mut service, Command::Status).daily_chain_goal;
    assert!(off.active.is_none());
    assert!(off.scheduled.is_none());
    assert!(!off.device_time_zone.is_empty());

    let scheduled = snapshot(
        &mut service,
        goal(2, None, Some("2026-11-24"), "Asia/Shanghai"),
    )
    .daily_chain_goal;
    assert!(scheduled.active.is_none());
    let change = scheduled.scheduled.unwrap();
    assert_eq!(change.effective_date, "2026-11-21");
    assert_eq!(change.settings.unwrap().quota, 2);
    for invalid in [
        goal(0, None, None, "Asia/Shanghai"),
        goal(2, None, None, "Not/AZone"),
        goal(2, Some("2026-11-19"), None, "Asia/Shanghai"),
        goal(2, Some("2026-11-22"), Some("2026-11-21"), "Asia/Shanghai"),
    ] {
        let before = snapshot(&mut service, Command::Status).daily_chain_goal;
        assert!(matches!(
            request(&mut service, invalid),
            Response::Error { .. }
        ));
        assert_eq!(
            snapshot(&mut service, Command::Status).daily_chain_goal,
            before
        );
    }

    clock.advance(3600);
    let active = snapshot(&mut service, Command::Status).daily_chain_goal;
    assert_eq!(active.date, "2026-11-21");
    assert_eq!(active.active.unwrap().quota, 2);
    let update = snapshot(
        &mut service,
        goal(3, Some("2026-11-21"), Some("2026-11-24"), "Asia/Shanghai"),
    )
    .daily_chain_goal;
    assert_eq!(update.active.unwrap().quota, 2);
    let change = update.scheduled.unwrap();
    assert_eq!(change.effective_date, "2026-11-22");
    assert_eq!(change.settings.unwrap().quota, 3);
    let disable = snapshot(
        &mut service,
        Command::ConfigureDailyChainGoal { settings: None },
    )
    .daily_chain_goal;
    assert_eq!(disable.active.unwrap().quota, 2);
    let change = disable.scheduled.unwrap();
    assert_eq!(change.effective_date, "2026-11-22");
    assert!(change.settings.is_none());
    clock.advance(86_400);
    let off = snapshot(&mut service, Command::Status).daily_chain_goal;
    assert!(off.active.is_none());
    assert!(off.scheduled.is_none());
}

fn start_round(service: &mut Service) {
    snapshot(
        service,
        Command::StartTitle {
            title: "Plan tomorrow".into(),
        },
    );
}

fn submit_success(service: &mut Service) -> Snapshot {
    snapshot(service, Command::ReviewSuccess { reflection: None })
}

#[test]
fn a_late_review_repairs_the_miss_on_its_original_completion_date() {
    let clock = ManualClock::new(at("2026-11-20T15:59:00Z"));
    let mut service = Service::new_with_clock(clock.clone());
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(SessionDurations::new(10, 5, 15).unwrap())
        .unwrap();
    for name in ["Claimed", "Unclaimed"] {
        snapshot(
            &mut service,
            Command::RewardCreate {
                name: name.into(),
                threshold: 1,
                budget: None,
            },
        );
    }
    snapshot(&mut service, goal(2, None, None, "Asia/Shanghai"));
    clock.advance(60);

    start_round(&mut service);
    clock.advance(10);
    let reviewed = submit_success(&mut service);
    assert_eq!(reviewed.daily_chain_goal.achieved, 1);
    assert_eq!(reviewed.action_chain.length, 1);
    let claimed = reviewed
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Claimed")
        .unwrap();
    let unclaimed = reviewed
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Unclaimed")
        .unwrap();
    snapshot(
        &mut service,
        Command::RewardClaim {
            unlock_id: claimed.id,
        },
    );
    snapshot(&mut service, Command::Skip);

    clock.advance(86_370);
    start_round(&mut service);
    clock.advance(25);
    service.tick();
    let missed = snapshot(&mut service, Command::Status);
    assert_eq!(missed.action_chain.length, 0);
    assert_eq!(missed.recent_ended_chains.len(), 1);
    let chain = &missed.recent_ended_chains[0];
    assert_eq!(
        chain.break_source,
        pomotui_protocol::ChainBreakSource::DailyChainGoal
    );
    let reason = chain.break_daily_chain_goal.as_ref().unwrap();
    assert_eq!(
        (reason.date.as_str(), reason.achieved, reason.quota),
        ("2026-11-21", 1, 2)
    );
    assert!(missed.pending_review.is_some());

    let repaired = submit_success(&mut service);
    assert_eq!(repaired.daily_chain_goal.date, "2026-11-22");
    assert_eq!(repaired.daily_chain_goal.achieved, 0);
    assert_eq!(repaired.action_chain.length, 2);
    assert!(repaired.recent_ended_chains.is_empty());
    assert!(
        repaired
            .current_chain_rewards
            .iter()
            .any(|reward| reward.id == claimed.id && reward.state == "claimed")
    );
    assert!(
        repaired
            .current_chain_rewards
            .iter()
            .any(|reward| reward.id == unclaimed.id && reward.state == "unlocked")
    );

    snapshot(&mut service, Command::Skip);
    start_round(&mut service);
    let stopped = snapshot(&mut service, Command::Stop);
    assert_ne!(stopped.recent_ended_chains[0].id, chain.id);
    assert!(matches!(
        request(&mut service, Command::EndedChainDelete { id: chain.id }),
        Response::Error { .. }
    ));
}

#[test]
fn inclusive_last_day_ends_at_the_actual_dst_boundary() {
    let clock = ManualClock::new(at("2026-03-07T12:00:00-05:00"));
    let mut service = Service::new_with_clock(clock.clone());
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(SessionDurations::new(10, 5, 15).unwrap())
        .unwrap();
    snapshot(
        &mut service,
        goal(2, None, Some("2026-03-08"), "America/New_York"),
    );
    clock.advance(12 * 3_600);
    start_round(&mut service);
    clock.advance(10);
    assert_eq!(submit_success(&mut service).daily_chain_goal.achieved, 1);
    clock.advance(23 * 3_600 - 11);
    let before = snapshot(&mut service, Command::Status);
    assert_eq!(before.daily_chain_goal.date, "2026-03-08");
    assert_eq!(before.daily_chain_goal.active.unwrap().quota, 2);
    assert_eq!(before.action_chain.length, 1);
    clock.advance(1);
    let at_cutoff = snapshot(&mut service, Command::Status);
    assert!(at_cutoff.daily_chain_goal.active.is_none());
    assert_eq!(at_cutoff.action_chain.length, 0);
    assert_eq!(
        at_cutoff.recent_ended_chains[0]
            .break_daily_chain_goal
            .as_ref()
            .unwrap()
            .date,
        "2026-03-08",
    );
}

#[test]
fn restart_catches_the_first_miss_and_fresh_start_clears_the_goal() {
    let database = std::env::temp_dir().join(format!(
        "pomotui-goal-{}.sqlite",
        RecordId::random().as_str()
    ));
    let clock = ManualClock::new(at("2026-11-20T23:00:00Z"));
    let mut service = Service::open_with_clock(&database, clock.clone()).unwrap();
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(SessionDurations::new(10, 5, 15).unwrap())
        .unwrap();
    for name in ["Claimed milestone", "Unclaimed milestone"] {
        snapshot(
            &mut service,
            Command::RewardCreate {
                name: name.into(),
                threshold: 1,
                budget: None,
            },
        );
    }
    snapshot(&mut service, goal(2, None, None, "UTC"));
    clock.advance(3_600);
    start_round(&mut service);
    clock.advance(10);
    let reviewed = submit_success(&mut service);
    assert_eq!(reviewed.current_chain_rewards.len(), 2);
    let claimed = reviewed
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Claimed milestone")
        .unwrap();
    snapshot(
        &mut service,
        Command::RewardClaim {
            unlock_id: claimed.id,
        },
    );
    drop(service);

    clock.advance(4 * 86_400);
    let mut service = Service::open_with_clock(&database, clock.clone()).unwrap();
    let missed = snapshot(&mut service, Command::Status);
    assert_eq!(missed.action_chain.length, 0);
    assert_eq!(missed.recent_ended_chains.len(), 1);
    let archive = &missed.recent_ended_chains[0];
    assert_eq!(
        archive.break_daily_chain_goal.as_ref().unwrap().date,
        "2026-11-21"
    );
    assert_eq!(archive.rewards.len(), 2);
    assert!(
        archive
            .rewards
            .iter()
            .any(|reward| reward.name == "Claimed milestone" && reward.state == "claimed")
    );
    assert!(
        archive
            .rewards
            .iter()
            .any(|reward| reward.name == "Unclaimed milestone" && reward.state == "unavailable")
    );
    drop(service);

    let mut service = Service::open_with_clock(&database, clock.clone()).unwrap();
    service.tick();
    let restarted = snapshot(&mut service, Command::Status);
    assert_eq!(restarted.recent_ended_chains, missed.recent_ended_chains);
    let fresh = snapshot(&mut service, Command::FreshStart { confirmed: true });
    assert!(fresh.daily_chain_goal.active.is_none());
    assert!(fresh.daily_chain_goal.scheduled.is_none());
    assert!(fresh.recent_ended_chains.is_empty());
    drop(service);

    std::fs::remove_file(database).unwrap();
}

#[test]
fn same_day_breaks_leave_credit_and_completion_at_midnight_starts_the_next_day() {
    let clock = ManualClock::new(at("2026-11-20T23:59:00Z"));
    let mut service = Service::new_with_clock(clock.clone());
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(SessionDurations::new(10, 5, 15).unwrap())
        .unwrap();
    snapshot(&mut service, goal(3, None, None, "UTC"));
    clock.advance(60);
    start_round(&mut service);
    clock.advance(10);
    submit_success(&mut service);
    snapshot(&mut service, Command::Skip);
    start_round(&mut service);
    let stopped = snapshot(&mut service, Command::Stop);
    assert_eq!(stopped.action_chain.length, 0);
    assert_eq!(stopped.daily_chain_goal.achieved, 1);
    start_round(&mut service);
    clock.advance(10);
    let reviewed = submit_success(&mut service);
    assert_eq!(reviewed.action_chain.length, 1);
    assert_eq!(reviewed.daily_chain_goal.achieved, 2);
    snapshot(&mut service, Command::Skip);

    clock.advance(86_370);
    start_round(&mut service);
    clock.advance(11);
    let tomorrow = submit_success(&mut service);
    assert_eq!(tomorrow.daily_chain_goal.date, "2026-11-22");
    assert_eq!(tomorrow.daily_chain_goal.achieved, 1);
    assert_eq!(tomorrow.action_chain.length, 1);
    assert_eq!(tomorrow.recent_ended_chains.len(), 2);
    let miss = tomorrow.recent_ended_chains[0]
        .break_daily_chain_goal
        .as_ref()
        .unwrap();
    assert_eq!(
        (miss.date.as_str(), miss.achieved, miss.quota),
        ("2026-11-21", 2, 3)
    );
}

#[test]
fn deleting_a_provisional_archive_hides_its_links_after_late_repair() {
    let database = std::env::temp_dir().join(format!(
        "pomotui-goal-delete-{}.sqlite",
        RecordId::random().as_str()
    ));
    let clock = ManualClock::new(at("2026-11-20T23:59:00Z"));
    let mut service = Service::open_with_clock(&database, clock.clone()).unwrap();
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(SessionDurations::new(10, 5, 15).unwrap())
        .unwrap();
    snapshot(&mut service, goal(2, None, None, "UTC"));
    clock.advance(60);
    start_round(&mut service);
    clock.advance(10);
    submit_success(&mut service);
    snapshot(&mut service, Command::Skip);

    clock.advance(86_370);
    start_round(&mut service);
    clock.advance(20);
    let missed = snapshot(&mut service, Command::Status);
    let deleted = snapshot(
        &mut service,
        Command::EndedChainDelete {
            id: missed.recent_ended_chains[0].id,
        },
    );
    assert!(deleted.recent_ended_chains.is_empty());
    let repaired = submit_success(&mut service);
    assert_eq!(repaired.action_chain.length, 0);
    assert!(repaired.recent_ended_chains.is_empty());
    drop(service);

    let mut service = Service::open_with_clock(&database, clock).unwrap();
    let restarted = snapshot(&mut service, Command::Status);
    assert_eq!(restarted.action_chain.length, 0);
    assert!(restarted.recent_ended_chains.is_empty());
    drop(service);
    std::fs::remove_file(database).unwrap();
}
