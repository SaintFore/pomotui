mod support;

use pomotui_domain::SessionDurations;
use pomotui_protocol::{
    Command, Handler, PROTOCOL_VERSION, Request, Response, SessionKind, Snapshot,
};
use pomotui_service::Service;
use pomotui_sync::RecordId;

fn command(service: &mut Service, command: Command) -> Snapshot {
    let response = service.handle(Request {
        version: PROTOCOL_VERSION,
        idempotency_key: command
            .mutates()
            .then(|| RecordId::random().as_str().into()),
        command,
    });
    let Response::Snapshot { snapshot } = response else {
        panic!("expected snapshot: {response:?}");
    };
    snapshot
}

#[test]
fn pause_and_duration_change_keep_the_original_round_eligible_for_review() {
    let clock = support::ManualClock::new(1_700_000_000);
    let mut service = Service::new_with_clock(clock.clone());
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(SessionDurations::new(50 * 60, 5 * 60, 15 * 60).unwrap())
        .unwrap();
    assert!(matches!(
        service.handle(Request {
            version: PROTOCOL_VERSION,
            idempotency_key: Some("create-task".into()),
            command: Command::TaskCreate {
                title: "Plan the next step".into()
            },
        }),
        Response::Data { .. }
    ));
    let focus = Command::Start {
        kind: SessionKind::Focus,
        task_id: Some(1),
    };
    assert_eq!(
        command(&mut service, focus.clone()).planned_seconds,
        50 * 60
    );
    clock.advance(20 * 60);
    assert_eq!(
        command(&mut service, Command::Pause).remaining_seconds,
        30 * 60
    );
    service
        .configure_durations(SessionDurations::new(25 * 60, 5 * 60, 15 * 60).unwrap())
        .unwrap();
    clock.advance(60 * 60);
    let paused = command(&mut service, Command::Status);
    assert_eq!(paused.remaining_seconds, 30 * 60);
    assert_eq!(paused.planned_seconds, 50 * 60);
    command(&mut service, Command::Resume);
    clock.advance(30 * 60);
    let completed = command(&mut service, Command::Status);
    assert_eq!(completed.action_chain.length, 0);
    assert_eq!(completed.pending_review.unwrap().actual_seconds, 50 * 60);
    assert_eq!(completed.completed_rounds, 1);
    let reviewed = command(&mut service, Command::ReviewSuccess { reflection: None });
    assert_eq!(reviewed.action_chain.length, 1);
    assert_eq!(reviewed.recent_chain_links[0].actual_seconds, 50 * 60);

    command(&mut service, Command::Skip);
    assert_eq!(command(&mut service, focus).planned_seconds, 25 * 60);
    clock.advance(25 * 60);
    let completed = command(&mut service, Command::Status);
    assert_eq!(completed.action_chain.length, 1);
    assert_eq!(completed.pending_review.unwrap().actual_seconds, 25 * 60);
    let reviewed = command(&mut service, Command::ReviewSuccess { reflection: None });
    assert_eq!(reviewed.action_chain.length, 2);
    assert_eq!(reviewed.recent_chain_links[1].actual_seconds, 25 * 60);
    assert_eq!(reviewed.completed_rounds, 2);

    command(
        &mut service,
        Command::Start {
            kind: SessionKind::ShortBreak,
            task_id: None,
        },
    );
    let stopped = command(&mut service, Command::Stop);
    assert_eq!(stopped.action_chain.length, 2);
    assert!(stopped.recent_ended_chains.is_empty());
    command(&mut service, Command::Skip);
    let skipped_focus = command(&mut service, Command::Skip);
    assert_eq!(skipped_focus.action_chain.length, 2);
    assert!(skipped_focus.recent_ended_chains.is_empty());
    assert_eq!(skipped_focus.recent_history[0].outcome, "Skipped");
}
