use pomotui_domain::{
    DomainEvent, History, SessionKind, SessionOutcome, SessionRecord, TaskError, TaskId,
    TaskStatus, TaskStore,
};

#[test]
fn duplicate_titles_require_explicit_identity() {
    let mut tasks = TaskStore::new();
    let first = tasks.create("Write spec").expect("create");
    let second = tasks.create("Write spec").expect("create");

    assert_eq!(
        tasks.resolve_title("Write spec"),
        Err(TaskError::AmbiguousTitle(vec![first, second]))
    );
}

#[test]
fn history_keeps_title_snapshot_after_task_rename_and_delete() {
    let mut tasks = TaskStore::new();
    let id = tasks.create("Original").expect("create");
    let event = DomainEvent::SessionEnded {
        kind: SessionKind::Focus,
        outcome: SessionOutcome::Completed,
        actual_seconds: 1_500,
        task_id: Some(id),
    };
    let record = SessionRecord::from_event(event, 1, 10_000, 1_500, &tasks);

    tasks.rename(id, "Renamed").expect("rename");
    tasks.delete(id, None).expect("delete");

    assert_eq!(record.task_title.as_deref(), Some("Original"));
    assert_eq!(record.task_id, Some(id));
}

#[test]
fn referenced_task_cannot_be_deleted_but_can_be_completed() {
    let mut tasks = TaskStore::new();
    let id = tasks.create("Current").expect("create");

    tasks.complete(id).expect("complete");
    assert_eq!(tasks.get(id).expect("task").status(), TaskStatus::Completed);
    assert_eq!(
        tasks.delete(id, Some(id)),
        Err(TaskError::ReferencedByCurrentSession(id))
    );
}

#[test]
fn task_titles_are_normalized_and_terminal_safe() {
    let mut tasks = TaskStore::new();
    let id = tasks.create("  Cafe\u{301}  ").expect("normalized title");
    assert_eq!(tasks.get(id).expect("task").title(), "Café");
    assert_eq!(
        tasks
            .resolve_title("Cafe\u{301}")
            .expect("normalized lookup"),
        id
    );

    for unsafe_title in [
        "line\nbreak",
        "escape\u{1b}[31m",
        "reversed\u{202e}title",
        "invisible\u{2063}separator",
    ] {
        assert!(matches!(
            tasks.create(unsafe_title),
            Err(TaskError::UnsafeTitleCharacter)
        ));
    }
}

#[test]
fn task_titles_have_independent_storage_and_display_limits() {
    let mut tasks = TaskStore::new();
    assert!(matches!(
        tasks.create("x".repeat(257)),
        Err(TaskError::TitleTooLong { .. })
    ));
    assert!(matches!(
        tasks.create("界".repeat(61)),
        Err(TaskError::TitleTooWide { .. })
    ));
}

#[test]
fn restoring_legacy_titles_validates_without_silently_normalizing() {
    let decomposed = "Cafe\u{301}".to_owned();
    let tasks = TaskStore::restore(
        vec![(TaskId::new(1), decomposed.clone(), TaskStatus::Open)],
        2,
    )
    .expect("safe legacy title");
    assert_eq!(tasks.get(TaskId::new(1)).expect("task").title(), decomposed);

    assert!(matches!(
        TaskStore::restore(
            vec![(TaskId::new(1), "bad\u{1b}".into(), TaskStatus::Open)],
            2
        ),
        Err(TaskError::UnsafeTitleCharacter)
    ));
}

#[test]
fn seven_day_summary_counts_actual_focus_and_only_completed_rounds() {
    let mut history = History::default();
    for (id, ended_at, outcome, seconds) in [
        (1, 50, SessionOutcome::Completed, 25),
        (2, 60, SessionOutcome::Stopped, 7),
        (3, 150, SessionOutcome::Skipped, 0),
    ] {
        history.push(SessionRecord {
            id,
            ended_at,
            kind: SessionKind::Focus,
            outcome,
            planned_seconds: 25,
            actual_seconds: seconds,
            task_id: Some(TaskId::new(1)),
            task_title: Some("A".into()),
        });
    }

    let summary = history.summarize([0, 100, 200, 300, 400, 500, 600, 700]);

    assert_eq!(summary.focus_seconds, [32, 0, 0, 0, 0, 0, 0]);
    assert_eq!(summary.completed_rounds, [1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(history.focus_seconds_for_task(TaskId::new(1)), 32);
}
