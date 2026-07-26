use pomotui_domain::{
    CurrentSession, DomainEvent, SessionDurations, SessionKind, SessionOutcome, TaskId, Timer,
};

fn timer(rounds_per_cycle: u8) -> Timer {
    Timer::new(
        SessionDurations::new(25, 5, 15).expect("valid durations"),
        rounds_per_cycle,
    )
    .expect("valid cycle")
}

#[test]
fn completed_focus_advances_cycle_and_reveals_short_break() {
    let mut timer = timer(4);
    timer.start(100, Some(TaskId::new(7))).expect("start");

    let transition = timer.advance(125).expect("deadline transition");

    assert_eq!(timer.focus_cycle().completed_rounds(), 1);
    assert_eq!(timer.current_task(), Some(TaskId::new(7)));
    assert!(matches!(
        timer.current_session(),
        CurrentSession::Pending(pending) if pending.kind() == SessionKind::ShortBreak
    ));
    assert_eq!(
        transition.events(),
        &[DomainEvent::SessionEnded {
            kind: SessionKind::Focus,
            outcome: SessionOutcome::Completed,
            actual_seconds: 25,
            task_id: Some(TaskId::new(7)),
        }]
    );
}

#[test]
fn pause_freezes_elapsed_time_until_explicit_resume() {
    let mut timer = timer(4);
    timer.start(10, None).expect("start");
    timer.pause(20).expect("pause");

    assert!(timer.advance(1_000).expect("advance").events().is_empty());
    assert_eq!(timer.remaining_seconds(1_000), 15);

    timer.resume(1_000).expect("resume");
    assert!(timer.advance(1_014).expect("advance").events().is_empty());
    assert_eq!(timer.remaining_seconds(1_014), 1);
    assert_eq!(
        timer.advance(1_015).expect("complete").events()[0],
        DomainEvent::SessionEnded {
            kind: SessionKind::Focus,
            outcome: SessionOutcome::Completed,
            actual_seconds: 25,
            task_id: None,
        }
    );
}

#[test]
fn stopping_preserves_recommendation_and_records_actual_time() {
    let mut timer = timer(4);
    timer.start(50, None).expect("start");

    let transition = timer.stop(57).expect("stop");

    assert_eq!(timer.focus_cycle().completed_rounds(), 0);
    assert!(matches!(
        timer.current_session(),
        CurrentSession::Pending(pending) if pending.kind() == SessionKind::Focus
    ));
    assert_eq!(
        transition.events()[0],
        DomainEvent::SessionEnded {
            kind: SessionKind::Focus,
            outcome: SessionOutcome::Stopped,
            actual_seconds: 7,
            task_id: None,
        }
    );
}

#[test]
fn skipped_focus_round_remains_due_after_break() {
    let mut timer = timer(4);

    let skipped = timer.skip().expect("skip focus");
    assert_eq!(
        skipped.events()[0],
        DomainEvent::SessionEnded {
            kind: SessionKind::Focus,
            outcome: SessionOutcome::Skipped,
            actual_seconds: 0,
            task_id: None,
        }
    );
    assert!(matches!(
        timer.current_session(),
        CurrentSession::Pending(pending) if pending.kind() == SessionKind::ShortBreak
    ));

    timer.start(0, None).expect("start break");
    timer.advance(5).expect("complete break");

    assert_eq!(timer.focus_cycle().completed_rounds(), 0);
    assert!(matches!(
        timer.current_session(),
        CurrentSession::Pending(pending) if pending.kind() == SessionKind::Focus
    ));
}

#[test]
fn configured_number_of_rounds_leads_to_long_break_and_completion_resets_cycle() {
    let mut timer = timer(2);

    for expected_round in 1..=2 {
        timer.start(100, None).expect("start focus");
        timer.advance(125).expect("complete focus");
        assert_eq!(timer.focus_cycle().completed_rounds(), expected_round);
        let expected_break = if expected_round == 2 {
            SessionKind::LongBreak
        } else {
            SessionKind::ShortBreak
        };
        assert!(matches!(
            timer.current_session(),
            CurrentSession::Pending(pending) if pending.kind() == expected_break
        ));
        timer.start(200, None).expect("start break");
        let deadline = if expected_break == SessionKind::LongBreak {
            215
        } else {
            205
        };
        timer.advance(deadline).expect("complete break");
    }

    assert_eq!(timer.focus_cycle().completed_rounds(), 0);
    assert!(matches!(
        timer.current_session(),
        CurrentSession::Pending(pending) if pending.kind() == SessionKind::Focus
    ));
}

#[test]
fn invalid_transitions_cannot_create_another_running_session() {
    let mut timer = timer(4);
    timer.start(0, None).expect("first start");

    assert!(timer.start(1, None).is_err());
    assert!(matches!(
        timer.current_session(),
        CurrentSession::Running(_)
    ));
}
