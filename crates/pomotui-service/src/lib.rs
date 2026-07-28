use pomotui_domain::{
    CurrentSession, History, SessionDurations, SessionKind as DomainKind, SessionOutcome,
    SessionRecord, SessionState, TaskError, TaskId, TaskStatus, TaskStore, Timer, TimerState,
    Transition,
};
use pomotui_platform::{
    Clock, DesktopReminder, LinuxClock, PendingReminderEffect, RecoveryObservation,
    ReminderDeliveryCounts, ReminderEffectKind, ReminderPort, SqliteRepository,
    elapsed_during_recovery,
};
use pomotui_protocol::{
    ActionChainSummary, Command, DurableHealth, DurableHealthState, Handler, PendingReviewSummary,
    ProtocolError, RecentSessionSummary, ReminderDelivery, Request, Response, SessionKind,
    Snapshot, TaskFocusSummary, TaskSummary, TodaySummary,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

const MAX_REMINDER_ATTEMPTS: u32 = 3;
const MAX_REMINDER_AGE_SECONDS: i64 = 60 * 60;

trait ServiceRepository: Send {
    fn save_state_once(&mut self, key: &str, payload: &str) -> Result<bool, String>;
    fn save_state(&mut self, payload: &str) -> Result<(), String>;
    fn save_completion(
        &mut self,
        payload: &str,
        reminder_key: &str,
        effects: &[ReminderEffectKind],
        created_at: i64,
    ) -> Result<bool, String>;
    fn due_reminder_effects(&self, now: i64) -> Result<Vec<PendingReminderEffect>, String>;
    fn acknowledge_reminder_effect(&mut self, id: i64, acknowledged_at: i64) -> Result<(), String>;
    fn record_reminder_failure(
        &mut self,
        id: i64,
        failed_at: i64,
        next_attempt_at: i64,
        exhausted: bool,
        error: &str,
    ) -> Result<(), String>;
    fn reminder_delivery_counts(&self) -> Result<ReminderDeliveryCounts, String>;
}

impl ServiceRepository for SqliteRepository {
    fn save_state_once(&mut self, key: &str, payload: &str) -> Result<bool, String> {
        self.save_state_once(key, payload)
            .map_err(|error| error.to_string())
    }

    fn save_state(&mut self, payload: &str) -> Result<(), String> {
        self.save_state(payload).map_err(|error| error.to_string())
    }

    fn save_completion(
        &mut self,
        payload: &str,
        reminder_key: &str,
        effects: &[ReminderEffectKind],
        created_at: i64,
    ) -> Result<bool, String> {
        self.save_completion(payload, reminder_key, effects, created_at)
            .map_err(|error| error.to_string())
    }

    fn due_reminder_effects(&self, now: i64) -> Result<Vec<PendingReminderEffect>, String> {
        self.due_reminder_effects(now)
            .map_err(|error| error.to_string())
    }

    fn acknowledge_reminder_effect(&mut self, id: i64, acknowledged_at: i64) -> Result<(), String> {
        self.acknowledge_reminder_effect(id, acknowledged_at)
            .map_err(|error| error.to_string())
    }

    fn record_reminder_failure(
        &mut self,
        id: i64,
        failed_at: i64,
        next_attempt_at: i64,
        exhausted: bool,
        error: &str,
    ) -> Result<(), String> {
        self.record_reminder_failure(id, failed_at, next_attempt_at, exhausted, error)
            .map_err(|error| error.to_string())
    }

    fn reminder_delivery_counts(&self) -> Result<ReminderDeliveryCounts, String> {
        self.reminder_delivery_counts()
            .map_err(|error| error.to_string())
    }
}

trait ReminderEffects: Send {
    fn configure(&mut self, sound: Option<std::path::PathBuf>, volume_percent: u8);
    fn notify(&mut self) -> Result<(), String>;
    fn play_sound(&mut self) -> Result<(), String>;
}

impl ReminderEffects for DesktopReminder {
    fn configure(&mut self, sound: Option<std::path::PathBuf>, volume_percent: u8) {
        self.sound = sound;
        self.volume_percent = volume_percent;
    }

    fn notify(&mut self) -> Result<(), String> {
        ReminderPort::notify(self).map_err(|error| error.to_string())
    }

    fn play_sound(&mut self) -> Result<(), String> {
        ReminderPort::play_sound(self).map_err(|error| error.to_string())
    }
}

pub struct Service {
    timer: Timer,
    tasks: TaskStore,
    history: History,
    applied_keys: std::collections::HashSet<String>,
    repository: Option<Box<dyn ServiceRepository>>,
    durable_health: DurableHealthState,
    last_successful_commit: Option<i64>,
    durable_error: Option<String>,
    reminder: Box<dyn ReminderEffects>,
    reminders_enabled: bool,
    sound_enabled: bool,
    next_event_id: u64,
    now: u64,
    wall: i64,
    current_chain_id: u64,
    current_chain_length: u64,
    pending_review: Option<PendingReviewState>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PendingReviewState {
    session_id: u64,
    actual_seconds: u64,
    task_id: Option<u64>,
    task_title: Option<String>,
}

impl Service {
    /// Creates the default v1 Timer Service state.
    ///
    /// # Panics
    ///
    /// Only panics if compile-time default durations become invalid.
    #[must_use]
    pub fn new() -> Self {
        let clock = LinuxClock;
        Self {
            timer: Timer::new(
                SessionDurations::new(25 * 60, 5 * 60, 15 * 60)
                    .expect("nonzero built-in durations"),
                4,
            )
            .expect("nonzero built-in cycle"),
            tasks: TaskStore::new(),
            history: History::default(),
            applied_keys: std::collections::HashSet::new(),
            repository: None,
            durable_health: DurableHealthState::Healthy,
            last_successful_commit: None,
            durable_error: None,
            reminder: Box::new(DesktopReminder {
                sound: None,
                volume_percent: 100,
            }),
            reminders_enabled: true,
            sound_enabled: false,
            next_event_id: 1,
            now: clock.monotonic_seconds().unwrap_or(0),
            wall: clock.wall_seconds().unwrap_or(0),
            current_chain_id: 1,
            current_chain_length: 0,
            pending_review: None,
        }
    }

    /// Opens or creates a durable Timer Service.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic database or state-validation error.
    pub fn open(path: &Path) -> Result<Self, String> {
        let repository = SqliteRepository::open(path).map_err(|error| error.to_string())?;
        let payload = repository
            .current_session_payload()
            .map_err(|error| error.to_string())?;
        let keys = repository
            .mutation_keys()
            .map_err(|error| error.to_string())?
            .into_iter()
            .collect();
        let had_payload = payload.is_some();
        let mut service = if let Some(payload) = payload.as_deref() {
            PersistedService::decode(payload)?
        } else {
            Self::new()
        };
        service.applied_keys = keys;
        service.repository = Some(Box::new(repository));
        if !had_payload {
            service.persist(None)?;
        }
        service.observe_time();
        Ok(service)
    }

    /// Applies defaults for Sessions that have not started yet.
    ///
    /// A Running or Paused Session retains its own planned duration.
    ///
    /// # Errors
    ///
    /// Returns an error if the updated durable state cannot be committed.
    pub fn configure_durations(&mut self, durations: SessionDurations) -> Result<(), String> {
        self.timer.set_durations(durations);
        self.persist(None)
    }

    /// Applies a new Focus Cycle length and persists it.
    ///
    /// # Errors
    ///
    /// Returns a domain validation or persistence error.
    pub fn configure_cycle(&mut self, rounds: u8) -> Result<(), String> {
        self.timer
            .set_rounds_per_cycle(rounds)
            .map_err(|error| error.to_string())?;
        self.persist(None)
    }

    pub fn configure_reminder(
        &mut self,
        enabled: bool,
        sound: Option<std::path::PathBuf>,
        volume_percent: u8,
    ) {
        self.reminders_enabled = enabled;
        self.sound_enabled = sound.is_some();
        self.reminder.configure(sound, volume_percent.min(100));
    }

    pub fn tick(&mut self) {
        self.observe_time();
        self.dispatch_pending_reminders();
    }

    fn snapshot(&self) -> Snapshot {
        let (state, kind, next_kind) = match self.timer.current_session() {
            CurrentSession::Pending(session) => {
                ("pending", session.kind(), Some(map_kind(session.kind())))
            }
            CurrentSession::Running(session) => (
                "running",
                session.kind(),
                Some(self.following_kind(session.kind())),
            ),
            CurrentSession::Paused(session) => (
                "paused",
                session.kind(),
                Some(self.following_kind(session.kind())),
            ),
        };
        let starts = local_day_boundaries(self.wall);
        let summary = self.history.summarize(starts);
        Snapshot {
            state: state.into(),
            kind: map_kind(kind),
            remaining_seconds: self.timer.remaining_seconds(self.now),
            planned_seconds: self.timer.planned_seconds(),
            current_task: self
                .timer
                .current_task()
                .and_then(|id| self.tasks.get(id).ok())
                .map(|task| task.title().to_owned()),
            current_task_id: self.timer.current_task().map(TaskId::get),
            completed_rounds: self.timer.focus_cycle().completed_rounds(),
            rounds_per_cycle: self.timer.focus_cycle().rounds_per_cycle(),
            next_kind,
            durable_health: DurableHealth {
                state: self.durable_health.clone(),
                last_successful_commit: self.last_successful_commit,
                error: self.durable_error.clone(),
            },
            reminder_delivery: self
                .repository
                .as_ref()
                .and_then(|repository| repository.reminder_delivery_counts().ok())
                .map_or_else(ReminderDelivery::default, |counts| ReminderDelivery {
                    pending: counts.pending,
                    retrying: counts.retrying,
                    delivered: counts.delivered,
                    exhausted: counts.exhausted,
                }),
            tasks: self
                .tasks
                .all()
                .iter()
                .map(|task| TaskSummary {
                    id: task.id().get(),
                    title: task.title().to_owned(),
                    completed: task.status() == TaskStatus::Completed,
                    focus_seconds: self.history.focus_seconds_for_task(task.id()),
                })
                .collect(),
            today: Box::new(TodaySummary {
                focus_seconds: summary.focus_seconds[6],
                completed_rounds: summary.completed_rounds[6],
                seven_day_focus_seconds: summary.focus_seconds,
                seven_day_dates: seven_day_dates(starts),
                average_focus_seconds: summary.average_focus_seconds(),
                task_focus: today_task_focus(&self.history, starts[6], starts[7]),
            }),
            recent_history: self
                .history
                .records()
                .iter()
                .rev()
                .map(|record| RecentSessionSummary {
                    id: record.id,
                    kind: map_kind(record.kind),
                    outcome: format!("{:?}", record.outcome),
                    actual_seconds: record.actual_seconds,
                    task_title: record.task_title.clone(),
                })
                .collect(),
            action_chain: ActionChainSummary {
                id: self.current_chain_id,
                length: self.current_chain_length,
            },
            pending_review: self
                .pending_review
                .as_ref()
                .map(|review| PendingReviewSummary {
                    session_id: review.session_id,
                    actual_seconds: review.actual_seconds,
                    task_id: review.task_id,
                    task_title: review.task_title.clone(),
                }),
        }
    }

    fn rejected(error: impl std::fmt::Display) -> Response {
        eprintln!("Timer Service rejected command: {error}");
        Response::Error {
            error: ProtocolError::Rejected {
                message: error.to_string(),
            },
        }
    }

    fn following_kind(&self, kind: DomainKind) -> SessionKind {
        match kind {
            DomainKind::Focus => {
                if self
                    .timer
                    .focus_cycle()
                    .completed_rounds()
                    .saturating_add(1)
                    >= self.timer.focus_cycle().rounds_per_cycle()
                {
                    SessionKind::LongBreak
                } else {
                    SessionKind::ShortBreak
                }
            }
            DomainKind::ShortBreak | DomainKind::LongBreak => SessionKind::Focus,
        }
    }

    fn observe_time(&mut self) {
        if self.durable_health == DurableHealthState::Degraded {
            return;
        }
        let clock = LinuxClock;
        let now = clock.monotonic_seconds().unwrap_or(self.now);
        let wall = clock.wall_seconds().unwrap_or(self.wall);
        self.apply_observation(now, wall);
    }

    fn apply_observation(&mut self, now: u64, wall: i64) {
        self.now = now;
        self.wall = wall;
        let planned = self.timer.planned_seconds();
        if let Ok(transition) = self.timer.advance(self.now) {
            let changed = !transition.events().is_empty();
            let completed = transition.events().iter().any(|event| {
                matches!(
                    event,
                    pomotui_domain::DomainEvent::SessionEnded {
                        outcome: SessionOutcome::Completed,
                        ..
                    }
                )
            });
            let reminder_key = format!("session-event-{}", self.next_event_id);
            self.record(&transition, planned);
            if completed {
                let effects = self.enabled_reminder_effects();
                if self
                    .persist_completion(&reminder_key, &effects)
                    .unwrap_or(false)
                    && self.reminders_enabled
                {
                    if self.repository.is_some() {
                        self.dispatch_pending_reminders();
                    } else {
                        self.dispatch_immediate_reminder(&effects);
                    }
                }
            } else if changed {
                let _persist_result = self.persist(None);
            }
        }
    }

    fn record(&mut self, transition: &Transition, planned_seconds: u64) {
        for event in transition.events() {
            let record = SessionRecord::from_event(
                *event,
                self.next_event_id,
                self.wall,
                planned_seconds,
                &self.tasks,
            );
            if matches!(
                event,
                pomotui_domain::DomainEvent::SessionEnded {
                    kind: DomainKind::Focus,
                    outcome: SessionOutcome::Completed,
                    ..
                }
            ) {
                self.pending_review = Some(PendingReviewState {
                    session_id: record.id,
                    actual_seconds: record.actual_seconds,
                    task_id: record.task_id.map(TaskId::get),
                    task_title: record.task_title.clone(),
                });
            }
            self.history.push(record);
            self.next_event_id = self.next_event_id.saturating_add(1);
        }
    }

    fn apply_transition(
        &mut self,
        result: Result<Transition, pomotui_domain::DomainError>,
        planned_seconds: u64,
    ) -> Result<(), String> {
        let transition = result.map_err(|error| error.to_string())?;
        self.record(&transition, planned_seconds);
        Ok(())
    }

    fn persist(&mut self, key: Option<&str>) -> Result<(), String> {
        let payload = PersistedService::encode(self)?;
        let Some(repository) = &mut self.repository else {
            return Ok(());
        };
        let result = if let Some(key) = key {
            repository.save_state_once(key, &payload).map(|_| ())
        } else {
            repository.save_state(&payload)
        };
        match result {
            Ok(()) => {
                if let Some(key) = key {
                    self.applied_keys.insert(key.to_owned());
                }
                self.last_successful_commit = Some(self.wall);
                Ok(())
            }
            Err(error) => {
                self.durable_health = DurableHealthState::Degraded;
                self.durable_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn persist_completion(
        &mut self,
        reminder_key: &str,
        effects: &[ReminderEffectKind],
    ) -> Result<bool, String> {
        let payload = PersistedService::encode(self)?;
        let Some(repository) = &mut self.repository else {
            return Ok(true);
        };
        match repository.save_completion(&payload, reminder_key, effects, self.wall) {
            Ok(claimed) => {
                self.last_successful_commit = Some(self.wall);
                Ok(claimed)
            }
            Err(error) => {
                self.durable_health = DurableHealthState::Degraded;
                self.durable_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn enabled_reminder_effects(&self) -> Vec<ReminderEffectKind> {
        if !self.reminders_enabled {
            return Vec::new();
        }
        let mut effects = vec![ReminderEffectKind::Notification];
        if self.sound_enabled {
            effects.push(ReminderEffectKind::Sound);
        }
        effects
    }

    fn dispatch_immediate_reminder(&mut self, effects: &[ReminderEffectKind]) {
        for effect in effects {
            let result = match effect {
                ReminderEffectKind::Notification => self.reminder.notify(),
                ReminderEffectKind::Sound => self.reminder.play_sound(),
            };
            if let Err(error) = result {
                eprintln!("Session Reminder {effect:?} failed: {error}");
            }
        }
    }

    fn dispatch_pending_reminders(&mut self) {
        if self.durable_health == DurableHealthState::Degraded {
            return;
        }
        let effects = match self
            .repository
            .as_ref()
            .map(|repository| repository.due_reminder_effects(self.wall))
        {
            None => return,
            Some(Ok(effects)) => effects,
            Some(Err(error)) => {
                self.mark_durable_failure(error);
                return;
            }
        };
        for effect in effects {
            let delivered = match effect.kind {
                ReminderEffectKind::Notification => self.reminder.notify(),
                ReminderEffectKind::Sound => self.reminder.play_sound(),
            };
            if let Err(error) = delivered {
                let attempt = effect.attempt_count.saturating_add(1);
                let exhausted = attempt >= MAX_REMINDER_ATTEMPTS
                    || self.wall.saturating_sub(effect.created_at) >= MAX_REMINDER_AGE_SECONDS;
                let base_delay = 5_i64
                    .saturating_mul(1_i64 << effect.attempt_count.min(6))
                    .min(300);
                let jitter = effect.id.rem_euclid(3);
                let next_attempt = self.wall.saturating_add(base_delay).saturating_add(jitter);
                let safe_error: String = error.chars().take(200).collect();
                let recorded = self
                    .repository
                    .as_mut()
                    .expect("repository exists while dispatching durable effects")
                    .record_reminder_failure(
                        effect.id,
                        self.wall,
                        next_attempt,
                        exhausted,
                        &safe_error,
                    );
                if let Err(error) = recorded {
                    self.mark_durable_failure(error);
                    return;
                }
                self.last_successful_commit = Some(self.wall);
                continue;
            }
            let acknowledged = self
                .repository
                .as_mut()
                .expect("repository exists while dispatching durable effects")
                .acknowledge_reminder_effect(effect.id, self.wall);
            if let Err(error) = acknowledged {
                self.mark_durable_failure(error);
                return;
            }
            self.last_successful_commit = Some(self.wall);
        }
    }

    fn mark_durable_failure(&mut self, error: String) {
        self.durable_health = DurableHealthState::Degraded;
        self.durable_error = Some(error);
    }

    fn task_rejected(error: TaskError) -> Response {
        let rule = match error {
            TaskError::EmptyTitle => Some(pomotui_protocol::TaskTitleRule::Empty),
            TaskError::UnsafeTitleCharacter => {
                Some(pomotui_protocol::TaskTitleRule::UnsafeCharacter)
            }
            TaskError::TitleTooLong { .. } => Some(pomotui_protocol::TaskTitleRule::TooLong),
            TaskError::TitleTooWide { .. } => Some(pomotui_protocol::TaskTitleRule::TooWide),
            _ => None,
        };
        if let Some(rule) = rule {
            Response::Error {
                error: ProtocolError::InvalidTaskTitle { rule },
            }
        } else {
            Self::rejected(error)
        }
    }

    fn durable_rejected(&self, error: String) -> Response {
        if self.durable_health == DurableHealthState::Degraded {
            Response::Error {
                error: ProtocolError::DurableWriteUnavailable { message: error },
            }
        } else {
            Self::rejected(error)
        }
    }
}

impl Default for Service {
    fn default() -> Self {
        Self::new()
    }
}

impl Handler for Service {
    #[allow(clippy::too_many_lines)]
    fn handle(&mut self, request: Request) -> Response {
        if self.durable_health == DurableHealthState::Degraded && request.command.mutates() {
            return Response::Error {
                error: ProtocolError::DurableWriteUnavailable {
                    message: self
                        .durable_error
                        .clone()
                        .unwrap_or_else(|| "durable state is unavailable".into()),
                },
            };
        }
        self.observe_time();
        let mutation_key = request.idempotency_key.clone();
        if let Some(key) = &mutation_key
            && self.applied_keys.contains(key)
        {
            return Response::Snapshot {
                snapshot: self.snapshot(),
            };
        }
        let result = match request.command {
            Command::Status => {
                return Response::Snapshot {
                    snapshot: self.snapshot(),
                };
            }
            Command::Start { kind, task_id } => {
                if kind == SessionKind::Focus && self.pending_review.is_some() {
                    return Self::rejected(
                        "Pending Review must be resolved before starting another Focus Session",
                    );
                }
                let current_kind = match self.timer.current_session() {
                    CurrentSession::Pending(session) => map_kind(session.kind()),
                    _ => kind.clone(),
                };
                if current_kind != kind {
                    Err(format!(
                        "recommended Session is {current_kind:?}, not {kind:?}"
                    ))
                } else if let Some(id) = task_id
                    && self.tasks.get(pomotui_domain::TaskId::new(id)).is_err()
                {
                    Err(format!("Task {id} does not exist"))
                } else {
                    let planned = self.timer.planned_seconds();
                    let transition = self
                        .timer
                        .start(self.now, task_id.map(pomotui_domain::TaskId::new));
                    self.apply_transition(transition, planned)
                }
            }
            Command::StartTitle { title } => {
                if self.pending_review.is_some() {
                    return Self::rejected(
                        "Pending Review must be resolved before starting another Focus Session",
                    );
                }
                let task_id = match self.tasks.resolve_title(&title) {
                    Ok(id) => Ok(id),
                    Err(pomotui_domain::TaskError::TitleNotFound(_)) => self.tasks.create(title),
                    Err(error) => Err(error),
                };
                match task_id {
                    Ok(task_id) => {
                        let planned = self.timer.planned_seconds();
                        let transition = self.timer.start(self.now, Some(task_id));
                        self.apply_transition(transition, planned)
                    }
                    Err(error) => return Self::task_rejected(error),
                }
            }
            Command::Pause => {
                let planned = self.timer.planned_seconds();
                let transition = self.timer.pause(self.now);
                self.apply_transition(transition, planned)
            }
            Command::Resume => {
                let planned = self.timer.planned_seconds();
                let transition = self.timer.resume(self.now);
                self.apply_transition(transition, planned)
            }
            Command::Stop => {
                let planned = self.timer.planned_seconds();
                let transition = self.timer.stop(self.now);
                self.apply_transition(transition, planned)
            }
            Command::Skip => {
                let planned = self.timer.planned_seconds();
                let transition = self.timer.skip();
                self.apply_transition(transition, planned)
            }
            Command::TaskCreate { title } => {
                return match self.tasks.create(title) {
                    Ok(id) => match self.persist(mutation_key.as_deref()) {
                        Ok(()) => Response::Data {
                            value: serde_json::json!({ "id": id.get() }),
                        },
                        Err(error) => self.durable_rejected(error),
                    },
                    Err(error) => Self::task_rejected(error),
                };
            }
            Command::TaskList => {
                return Response::Data {
                    value: serde_json::Value::Array(
                        self.tasks
                            .all()
                            .iter()
                            .map(|task| {
                                serde_json::json!({
                                    "id": task.id().get(),
                                    "title": task.title(),
                                    "status": format!("{:?}", task.status()).to_lowercase()
                                })
                            })
                            .collect(),
                    ),
                };
            }
            Command::TaskRename { id, title } => {
                return match self.tasks.rename(pomotui_domain::TaskId::new(id), title) {
                    Ok(()) => match self.persist(mutation_key.as_deref()) {
                        Ok(()) => Response::Snapshot {
                            snapshot: self.snapshot(),
                        },
                        Err(error) => self.durable_rejected(error),
                    },
                    Err(error) => Self::task_rejected(error),
                };
            }
            Command::TaskComplete { id } => self
                .tasks
                .complete(pomotui_domain::TaskId::new(id))
                .map_err(|error| error.to_string()),
            Command::TaskReopen { id } => self
                .tasks
                .reopen(pomotui_domain::TaskId::new(id))
                .map_err(|error| error.to_string()),
            Command::TaskDelete { id } => {
                let id = pomotui_domain::TaskId::new(id);
                self.tasks
                    .get(id)
                    .map_err(|error| error.to_string())
                    .and_then(|_| {
                        self.timer
                            .detach_pending_task(id)
                            .map_err(|error| error.to_string())
                    })
                    .and_then(|()| {
                        self.tasks
                            .delete(id, self.timer.current_task())
                            .map(drop)
                            .map_err(|error| error.to_string())
                    })
            }
            Command::TaskSelect { id, stop_current } => {
                let id = pomotui_domain::TaskId::new(id);
                if let Err(error) = self.tasks.get(id) {
                    Err(error.to_string())
                } else if matches!(self.timer.current_session(), CurrentSession::Pending(_)) {
                    self.timer
                        .select_pending_task(id)
                        .map_err(|error| error.to_string())
                } else if stop_current {
                    let planned = self.timer.planned_seconds();
                    let transition = self.timer.stop(self.now);
                    self.apply_transition(transition, planned).and_then(|()| {
                        self.timer
                            .select_pending_task(id)
                            .map_err(|error| error.to_string())
                    })
                } else {
                    Err("Current Session must be stopped before switching Task".into())
                }
            }
            Command::History => {
                return Response::Data {
                    value: serde_json::Value::Array(
                        self.history
                            .records()
                            .iter()
                            .map(|record| {
                                serde_json::json!({
                                    "ended_at": record.ended_at,
                                    "kind": format!("{:?}", record.kind),
                                    "outcome": format!("{:?}", record.outcome),
                                    "planned_seconds": record.planned_seconds,
                                    "actual_seconds": record.actual_seconds,
                                    "task_id": record.task_id.map(pomotui_domain::TaskId::get),
                                    "task_title": record.task_title,
                                })
                            })
                            .collect(),
                    ),
                };
            }
            Command::HistoryDelete { ids } => {
                if ids.is_empty() {
                    Err("Select at least one Session History entry".into())
                } else if self.history.delete(&ids) == 0 {
                    Err("Selected Session History entries no longer exist".into())
                } else {
                    Ok(())
                }
            }
            Command::Summary => {
                let starts = local_day_boundaries(self.wall);
                let summary = self.history.summarize(starts);
                return Response::Data {
                    value: serde_json::json!({
                        "focus_seconds": summary.focus_seconds,
                        "completed_rounds": summary.completed_rounds,
                        "average_focus_seconds": summary.average_focus_seconds(),
                    }),
                };
            }
        };
        match result {
            Ok(()) => match self.persist(mutation_key.as_deref()) {
                Ok(()) => Response::Snapshot {
                    snapshot: self.snapshot(),
                },
                Err(error) => self.durable_rejected(error),
            },
            Err(error) => Self::rejected(error),
        }
    }
}

fn map_kind(kind: pomotui_domain::SessionKind) -> SessionKind {
    match kind {
        pomotui_domain::SessionKind::Focus => SessionKind::Focus,
        pomotui_domain::SessionKind::ShortBreak => SessionKind::ShortBreak,
        pomotui_domain::SessionKind::LongBreak => SessionKind::LongBreak,
    }
}

fn local_day_boundaries(wall: i64) -> [i64; 8] {
    use chrono::{Local, TimeZone};
    let now = Local
        .timestamp_opt(wall, 0)
        .single()
        .unwrap_or_else(Local::now);
    let today = now.date_naive();
    std::array::from_fn(|index| {
        let offset = i64::try_from(index).expect("eight boundaries fit i64") - 6;
        let date = today
            .checked_add_signed(chrono::Duration::days(offset))
            .expect("nearby local day is representable");
        Local
            .from_local_datetime(
                &date
                    .and_hms_opt(0, 0, 0)
                    .expect("midnight is a valid naive time"),
            )
            .earliest()
            .map_or(wall, |boundary| boundary.timestamp())
    })
}

fn seven_day_dates(starts: [i64; 8]) -> [String; 7] {
    use chrono::{Local, TimeZone};
    std::array::from_fn(|index| {
        Local
            .timestamp_opt(starts[index], 0)
            .earliest()
            .map_or_else(
                || "---- -- --".into(),
                |date| date.format("%Y-%m-%d").to_string(),
            )
    })
}

fn today_task_focus(history: &History, start: i64, end: i64) -> Vec<TaskFocusSummary> {
    let mut totals = std::collections::BTreeMap::<Option<String>, u64>::new();
    for record in history.records() {
        if record.kind == DomainKind::Focus
            && record.ended_at >= start
            && record.ended_at < end
            && record.actual_seconds > 0
        {
            *totals.entry(record.task_title.clone()).or_default() += record.actual_seconds;
        }
    }
    let mut summaries = totals
        .into_iter()
        .map(|(task_title, focus_seconds)| TaskFocusSummary {
            task_title,
            focus_seconds,
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .focus_seconds
            .cmp(&left.focus_seconds)
            .then_with(|| left.task_title.cmp(&right.task_title))
    });
    summaries
}

#[derive(Deserialize, Serialize)]
struct PersistedService {
    timer: PersistedTimer,
    tasks: Vec<PersistedTask>,
    next_task_id: u64,
    history: Vec<PersistedRecord>,
    next_event_id: u64,
    boot_id: String,
    observed_monotonic: u64,
    observed_wall: i64,
    #[serde(default = "default_chain_id")]
    current_chain_id: u64,
    #[serde(default)]
    current_chain_length: u64,
    #[serde(default)]
    pending_review: Option<PendingReviewState>,
}

const fn default_chain_id() -> u64 {
    1
}

#[derive(Deserialize, Serialize)]
struct PersistedTimer {
    session: PersistedSession,
    current_task: Option<u64>,
    completed_rounds: u8,
    rounds_per_cycle: u8,
    focus_seconds: u64,
    short_break_seconds: u64,
    long_break_seconds: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum PersistedSession {
    Pending {
        kind: String,
    },
    Running {
        kind: String,
        planned_seconds: u64,
        accumulated_seconds: u64,
        started_at: u64,
        task_id: Option<u64>,
    },
    Paused {
        kind: String,
        planned_seconds: u64,
        elapsed_seconds: u64,
        task_id: Option<u64>,
    },
}

#[derive(Deserialize, Serialize)]
struct PersistedTask {
    id: u64,
    title: String,
    status: String,
}

#[derive(Deserialize, Serialize)]
struct PersistedRecord {
    #[serde(default)]
    id: u64,
    ended_at: i64,
    kind: String,
    outcome: String,
    planned_seconds: u64,
    actual_seconds: u64,
    task_id: Option<u64>,
    task_title: Option<String>,
}

impl PersistedService {
    fn encode(service: &Service) -> Result<String, String> {
        let state = service.timer.state();
        let session = match state.session {
            SessionState::Pending { kind } => PersistedSession::Pending {
                kind: domain_kind_name(kind).into(),
            },
            SessionState::Running {
                kind,
                planned_seconds,
                accumulated_seconds,
                started_at,
                task_id,
            } => PersistedSession::Running {
                kind: domain_kind_name(kind).into(),
                planned_seconds,
                accumulated_seconds: accumulated_seconds
                    .saturating_add(service.now.saturating_sub(started_at)),
                started_at: service.now,
                task_id: task_id.map(TaskId::get),
            },
            SessionState::Paused {
                kind,
                planned_seconds,
                elapsed_seconds,
                task_id,
            } => PersistedSession::Paused {
                kind: domain_kind_name(kind).into(),
                planned_seconds,
                elapsed_seconds,
                task_id: task_id.map(TaskId::get),
            },
        };
        let clock = LinuxClock;
        let persisted = Self {
            timer: PersistedTimer {
                session,
                current_task: state.current_task.map(TaskId::get),
                completed_rounds: state.completed_rounds,
                rounds_per_cycle: state.rounds_per_cycle,
                focus_seconds: state.durations.focus(),
                short_break_seconds: state.durations.short_break(),
                long_break_seconds: state.durations.long_break(),
            },
            tasks: service
                .tasks
                .all()
                .iter()
                .map(|task| PersistedTask {
                    id: task.id().get(),
                    title: task.title().to_owned(),
                    status: match task.status() {
                        TaskStatus::Open => "open",
                        TaskStatus::Completed => "completed",
                    }
                    .into(),
                })
                .collect(),
            next_task_id: service.tasks.next_id(),
            history: service
                .history
                .records()
                .iter()
                .map(|record| PersistedRecord {
                    id: record.id,
                    ended_at: record.ended_at,
                    kind: domain_kind_name(record.kind).into(),
                    outcome: outcome_name(record.outcome).into(),
                    planned_seconds: record.planned_seconds,
                    actual_seconds: record.actual_seconds,
                    task_id: record.task_id.map(TaskId::get),
                    task_title: record.task_title.clone(),
                })
                .collect(),
            next_event_id: service.next_event_id,
            boot_id: clock.boot_id().unwrap_or_else(|_| "unknown".into()),
            observed_monotonic: service.now,
            observed_wall: service.wall,
            current_chain_id: service.current_chain_id,
            current_chain_length: service.current_chain_length,
            pending_review: service.pending_review.clone(),
        };
        serde_json::to_string(&persisted).map_err(|error| error.to_string())
    }

    #[allow(clippy::too_many_lines)]
    fn decode(payload: &str) -> Result<Service, String> {
        let persisted: Self = serde_json::from_str(payload)
            .map_err(|error| format!("invalid durable state: {error}"))?;
        let durations = SessionDurations::new(
            persisted.timer.focus_seconds,
            persisted.timer.short_break_seconds,
            persisted.timer.long_break_seconds,
        )
        .map_err(|error| error.to_string())?;
        let clock = LinuxClock;
        let current_observation = RecoveryObservation {
            boot_id: clock
                .boot_id()
                .unwrap_or_else(|_| persisted.boot_id.clone()),
            monotonic_seconds: clock
                .monotonic_seconds()
                .unwrap_or(persisted.observed_monotonic),
            wall_seconds: clock.wall_seconds().unwrap_or(persisted.observed_wall),
        };
        let recovery = elapsed_during_recovery(
            &RecoveryObservation {
                boot_id: persisted.boot_id.clone(),
                monotonic_seconds: persisted.observed_monotonic,
                wall_seconds: persisted.observed_wall,
            },
            &current_observation,
        );
        eprintln!(
            "Timer Service recovery: {:?}, elapsed={}s",
            recovery.source, recovery.seconds
        );
        let recovery_elapsed = recovery.seconds;
        let session = match persisted.timer.session {
            PersistedSession::Pending { kind } => SessionState::Pending {
                kind: parse_kind(&kind)?,
            },
            PersistedSession::Running {
                kind,
                planned_seconds,
                accumulated_seconds,
                started_at: _,
                task_id,
            } => SessionState::Running {
                kind: parse_kind(&kind)?,
                planned_seconds,
                accumulated_seconds: accumulated_seconds.saturating_add(recovery_elapsed),
                started_at: current_observation.monotonic_seconds,
                task_id: task_id.map(TaskId::new),
            },
            PersistedSession::Paused {
                kind,
                planned_seconds,
                elapsed_seconds,
                task_id,
            } => SessionState::Paused {
                kind: parse_kind(&kind)?,
                planned_seconds,
                elapsed_seconds,
                task_id: task_id.map(TaskId::new),
            },
        };
        let timer = Timer::restore(TimerState {
            session,
            current_task: persisted.timer.current_task.map(TaskId::new),
            completed_rounds: persisted.timer.completed_rounds,
            rounds_per_cycle: persisted.timer.rounds_per_cycle,
            durations,
        })
        .map_err(|error| error.to_string())?;
        let tasks = TaskStore::restore(
            persisted
                .tasks
                .into_iter()
                .map(|task| {
                    let status = match task.status.as_str() {
                        "open" => Ok(TaskStatus::Open),
                        "completed" => Ok(TaskStatus::Completed),
                        other => Err(format!("invalid Task status: {other}")),
                    }?;
                    Ok((TaskId::new(task.id), task.title, status))
                })
                .collect::<Result<_, String>>()?,
            persisted.next_task_id,
        )
        .map_err(|error| error.to_string())?;
        let history = History::restore(
            persisted
                .history
                .into_iter()
                .enumerate()
                .map(|(index, record)| {
                    Ok(SessionRecord {
                        id: if record.id == 0 {
                            u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1)
                        } else {
                            record.id
                        },
                        ended_at: record.ended_at,
                        kind: parse_kind(&record.kind)?,
                        outcome: parse_outcome(&record.outcome)?,
                        planned_seconds: record.planned_seconds,
                        actual_seconds: record.actual_seconds,
                        task_id: record.task_id.map(TaskId::new),
                        task_title: record.task_title,
                    })
                })
                .collect::<Result<_, String>>()?,
        );
        Ok(Service {
            timer,
            tasks,
            history,
            applied_keys: std::collections::HashSet::new(),
            repository: None,
            durable_health: DurableHealthState::Healthy,
            last_successful_commit: None,
            durable_error: None,
            reminder: Box::new(DesktopReminder {
                sound: None,
                volume_percent: 100,
            }),
            reminders_enabled: true,
            sound_enabled: false,
            next_event_id: persisted.next_event_id.max(1),
            now: current_observation.monotonic_seconds,
            wall: current_observation.wall_seconds,
            current_chain_id: persisted.current_chain_id,
            current_chain_length: persisted.current_chain_length,
            pending_review: persisted.pending_review,
        })
    }
}

const fn domain_kind_name(kind: DomainKind) -> &'static str {
    match kind {
        DomainKind::Focus => "focus",
        DomainKind::ShortBreak => "short_break",
        DomainKind::LongBreak => "long_break",
    }
}

fn parse_kind(value: &str) -> Result<DomainKind, String> {
    match value {
        "focus" => Ok(DomainKind::Focus),
        "short_break" => Ok(DomainKind::ShortBreak),
        "long_break" => Ok(DomainKind::LongBreak),
        _ => Err(format!("invalid Session kind: {value}")),
    }
}

const fn outcome_name(outcome: SessionOutcome) -> &'static str {
    match outcome {
        SessionOutcome::Completed => "completed",
        SessionOutcome::Stopped => "stopped",
        SessionOutcome::Skipped => "skipped",
    }
}

fn parse_outcome(value: &str) -> Result<SessionOutcome, String> {
    match value {
        "completed" => Ok(SessionOutcome::Completed),
        "stopped" => Ok(SessionOutcome::Stopped),
        "skipped" => Ok(SessionOutcome::Skipped),
        _ => Err(format!("invalid Session outcome: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pomotui_protocol::{PROTOCOL_VERSION, TaskTitleRule};

    fn request(key: Option<&str>, command: Command) -> Request {
        Request {
            version: PROTOCOL_VERSION,
            idempotency_key: key.map(str::to_owned),
            command,
        }
    }

    fn database_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "pomotui-service-{}-{:?}.sqlite3",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn restart_recovers_tasks_current_session_history_and_idempotency() {
        let path = database_path();
        let _ = std::fs::remove_file(&path);
        {
            let mut service = Service::open(&path).expect("first service");
            let created = service.handle(request(
                Some("create-1"),
                Command::TaskCreate {
                    title: "Durable Task".into(),
                },
            ));
            assert!(matches!(created, Response::Data { .. }));
            service.handle(request(
                Some("start-1"),
                Command::Start {
                    kind: SessionKind::Focus,
                    task_id: Some(1),
                },
            ));
            service.handle(request(Some("stop-1"), Command::Stop));
        }
        {
            let mut service = Service::open(&path).expect("restarted service");
            let replay = service.handle(request(
                Some("create-1"),
                Command::TaskCreate {
                    title: "Duplicate".into(),
                },
            ));
            assert!(matches!(replay, Response::Snapshot { .. }));
            let Response::Data { value: tasks } = service.handle(request(None, Command::TaskList))
            else {
                panic!("task list response");
            };
            assert_eq!(tasks.as_array().expect("array").len(), 1);
            assert_eq!(tasks[0]["title"], "Durable Task");
            let Response::Data { value: history } = service.handle(request(None, Command::History))
            else {
                panic!("history response");
            };
            assert_eq!(history.as_array().expect("array").len(), 1);
            assert_eq!(history[0]["outcome"], "Stopped");
            assert_eq!(
                service.snapshot().recent_history[0].task_title.as_deref(),
                Some("Durable Task")
            );
        }
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn deadline_tick_commits_completion_once_without_a_frontend() {
        let path = database_path().with_extension("deadline.sqlite3");
        let _ = std::fs::remove_file(&path);
        let mut service = Service::open(&path).expect("service");
        service.reminders_enabled = false;
        service
            .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
            .expect("configure");
        let start = service.now;
        let transition = service.timer.start(start, None);
        service
            .apply_transition(transition, 1)
            .expect("start transition");
        service
            .persist(Some("start-deadline"))
            .expect("persist start");

        service.apply_observation(start + 1, service.wall + 1);
        service.apply_observation(start + 100, service.wall + 100);

        assert_eq!(service.history.records().len(), 1);
        assert_eq!(
            service.history.records()[0].outcome,
            SessionOutcome::Completed
        );
        assert_eq!(service.timer.focus_cycle().completed_rounds(), 1);
        drop(service);

        let restarted = Service::open(&path).expect("restart");
        assert_eq!(restarted.history.records().len(), 1);
        assert_eq!(restarted.timer.focus_cycle().completed_rounds(), 1);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn completed_focus_requires_review_before_another_focus_but_allows_break() {
        let mut service = Service::new();
        service.reminders_enabled = false;
        service
            .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
            .expect("configure");
        let start = service.now;

        assert!(matches!(
            service.handle(request(
                Some("start-focus"),
                Command::Start {
                    kind: SessionKind::Focus,
                    task_id: None,
                },
            )),
            Response::Snapshot { .. }
        ));
        service.apply_observation(start + 1, service.wall + 1);

        let Response::Snapshot { snapshot } = service.handle(request(None, Command::Status)) else {
            panic!("status");
        };
        assert_eq!(snapshot.action_chain.length, 0);
        assert!(snapshot.pending_review.is_some());

        assert!(matches!(
            service.handle(request(
                Some("start-break"),
                Command::Start {
                    kind: SessionKind::ShortBreak,
                    task_id: None,
                },
            )),
            Response::Snapshot { .. }
        ));
        service.apply_observation(start + 2, service.wall + 2);

        assert!(matches!(
            service.handle(request(
                Some("blocked-focus"),
                Command::Start {
                    kind: SessionKind::Focus,
                    task_id: None,
                },
            )),
            Response::Error {
                error: ProtocolError::Rejected { message }
            } if message == "Pending Review must be resolved before starting another Focus Session"
        ));
    }

    #[test]
    fn start_by_title_creates_new_task_and_rejects_ambiguous_existing_titles() {
        let mut service = Service::new();
        let response = service.handle(request(
            Some("new-title"),
            Command::StartTitle {
                title: "New work".into(),
            },
        ));
        assert!(matches!(response, Response::Snapshot { .. }));
        assert_eq!(service.tasks.all().len(), 1);
        service.handle(request(
            Some("complete-current-task"),
            Command::TaskComplete { id: 1 },
        ));
        assert_eq!(service.timer.current_task(), Some(TaskId::new(1)));
        assert!(matches!(
            service.timer.current_session(),
            CurrentSession::Running(_)
        ));
        service.handle(request(Some("stop-new"), Command::Stop));

        service.tasks.create("Duplicate").expect("first duplicate");
        service.tasks.create("Duplicate").expect("second duplicate");
        let response = service.handle(request(
            Some("ambiguous"),
            Command::StartTitle {
                title: "Duplicate".into(),
            },
        ));
        let Response::Error {
            error: ProtocolError::Rejected { message },
        } = response
        else {
            panic!("ambiguous rejection");
        };
        assert!(message.contains("AmbiguousTitle"));
        assert!(message.contains("TaskId"));
    }

    #[test]
    fn task_title_rejections_are_stable_and_do_not_persist_unsafe_text() {
        let mut service = Service::new();
        for (key, command) in [
            (
                "unsafe-create",
                Command::TaskCreate {
                    title: "bad\u{1b}[31m".into(),
                },
            ),
            (
                "unsafe-start",
                Command::StartTitle {
                    title: "bad\u{202e}title".into(),
                },
            ),
        ] {
            assert_eq!(
                service.handle(request(Some(key), command)),
                Response::Error {
                    error: ProtocolError::InvalidTaskTitle {
                        rule: TaskTitleRule::UnsafeCharacter
                    }
                }
            );
        }
        assert!(service.tasks.all().is_empty());

        service.handle(request(
            Some("safe-create"),
            Command::TaskCreate {
                title: "Safe".into(),
            },
        ));
        assert_eq!(
            service.handle(request(
                Some("unsafe-rename"),
                Command::TaskRename {
                    id: 1,
                    title: "line\nbreak".into(),
                },
            )),
            Response::Error {
                error: ProtocolError::InvalidTaskTitle {
                    rule: TaskTitleRule::UnsafeCharacter
                }
            }
        );
        assert_eq!(
            service.tasks.get(TaskId::new(1)).expect("task").title(),
            "Safe"
        );
    }

    #[test]
    fn durable_write_failure_is_visible_and_blocks_later_mutations() {
        let mut service = Service::new();
        service.repository = Some(Box::new(FailingRepository {
            successful_writes_remaining: 1,
            inner: None,
        }));
        service.persist(None).expect("initial durable commit");

        assert!(matches!(
            service.handle(request(
                Some("first-write-failure"),
                Command::TaskCreate {
                    title: "Volatile".into(),
                },
            )),
            Response::Error {
                error: ProtocolError::DurableWriteUnavailable { .. }
            }
        ));

        let Response::Snapshot { snapshot } = service.handle(request(None, Command::Status)) else {
            panic!("status remains available");
        };
        assert_eq!(
            snapshot.durable_health.state,
            pomotui_protocol::DurableHealthState::Degraded
        );
        assert_eq!(snapshot.tasks.len(), 1);
        assert!(snapshot.durable_health.last_successful_commit.is_some());

        assert!(matches!(
            service.handle(request(
                Some("blocked-mutation"),
                Command::TaskCreate {
                    title: "Must not appear".into(),
                },
            )),
            Response::Error {
                error: ProtocolError::DurableWriteUnavailable { .. }
            }
        ));
        let Response::Data { value } = service.handle(request(None, Command::TaskList)) else {
            panic!("Task list remains available");
        };
        assert_eq!(value.as_array().expect("tasks").len(), 1);
    }

    struct FailingRepository {
        successful_writes_remaining: usize,
        inner: Option<SqliteRepository>,
    }

    impl ServiceRepository for FailingRepository {
        fn save_state_once(&mut self, key: &str, payload: &str) -> Result<bool, String> {
            self.write()?;
            self.inner.as_mut().map_or(Ok(true), |inner| {
                inner
                    .save_state_once(key, payload)
                    .map_err(|error| error.to_string())
            })
        }

        fn save_state(&mut self, payload: &str) -> Result<(), String> {
            self.write()?;
            self.inner.as_mut().map_or(Ok(()), |inner| {
                inner.save_state(payload).map_err(|error| error.to_string())
            })
        }

        fn save_completion(
            &mut self,
            payload: &str,
            reminder_key: &str,
            effects: &[ReminderEffectKind],
            created_at: i64,
        ) -> Result<bool, String> {
            self.write()?;
            self.inner.as_mut().map_or(Ok(true), |inner| {
                inner
                    .save_completion(payload, reminder_key, effects, created_at)
                    .map_err(|error| error.to_string())
            })
        }

        fn due_reminder_effects(&self, now: i64) -> Result<Vec<PendingReminderEffect>, String> {
            self.inner.as_ref().map_or(Ok(Vec::new()), |inner| {
                inner
                    .due_reminder_effects(now)
                    .map_err(|error| error.to_string())
            })
        }

        fn acknowledge_reminder_effect(
            &mut self,
            id: i64,
            acknowledged_at: i64,
        ) -> Result<(), String> {
            self.write()?;
            self.inner.as_mut().map_or(Ok(()), |inner| {
                inner
                    .acknowledge_reminder_effect(id, acknowledged_at)
                    .map_err(|error| error.to_string())
            })
        }

        fn record_reminder_failure(
            &mut self,
            id: i64,
            failed_at: i64,
            next_attempt_at: i64,
            exhausted: bool,
            error: &str,
        ) -> Result<(), String> {
            self.write()?;
            self.inner.as_mut().map_or(Ok(()), |inner| {
                inner
                    .record_reminder_failure(id, failed_at, next_attempt_at, exhausted, error)
                    .map_err(|error| error.to_string())
            })
        }

        fn reminder_delivery_counts(&self) -> Result<ReminderDeliveryCounts, String> {
            self.inner
                .as_ref()
                .map_or(Ok(ReminderDeliveryCounts::default()), |inner| {
                    inner
                        .reminder_delivery_counts()
                        .map_err(|error| error.to_string())
                })
        }
    }

    #[test]
    fn restart_recovers_last_commit_after_a_degraded_mutation() {
        let path = database_path().with_extension("degraded.sqlite3");
        let _ = std::fs::remove_file(&path);
        let mut service = Service::open(&path).expect("service");
        service.repository = Some(Box::new(FailingRepository {
            successful_writes_remaining: 0,
            inner: Some(SqliteRepository::open(&path).expect("failure-injected repository")),
        }));

        assert!(matches!(
            service.handle(request(
                Some("volatile-task"),
                Command::TaskCreate {
                    title: "Not durable".into(),
                },
            )),
            Response::Error {
                error: ProtocolError::DurableWriteUnavailable { .. }
            }
        ));
        drop(service);

        let restarted = Service::open(&path).expect("restart from last commit");
        assert!(restarted.snapshot().tasks.is_empty());
        assert_eq!(
            restarted.snapshot().durable_health.state,
            DurableHealthState::Healthy
        );
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn deadline_write_failure_degrades_and_freezes_progression() {
        let mut service = Service::new();
        service.reminders_enabled = false;
        service
            .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
            .expect("configure");
        let start = service.now;
        let transition = service.timer.start(start, None);
        service
            .apply_transition(transition, 1)
            .expect("start transition");
        service.repository = Some(Box::new(FailingRepository {
            successful_writes_remaining: 0,
            inner: None,
        }));

        service.apply_observation(start + 1, service.wall + 1);
        assert_eq!(service.durable_health, DurableHealthState::Degraded);
        assert_eq!(service.history.records().len(), 1);

        service.apply_observation(start + 100, service.wall + 100);
        assert_eq!(service.history.records().len(), 1);
        assert_eq!(service.timer.focus_cycle().completed_rounds(), 1);
    }

    #[test]
    fn reminder_effects_are_independent_and_recover_after_restart() {
        let path = database_path().with_extension("outbox.sqlite3");
        let _ = std::fs::remove_file(&path);
        let first_attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        {
            let mut service = Service::open(&path).expect("service");
            service.reminder = Box::new(RecordingReminder {
                attempts: std::sync::Arc::clone(&first_attempts),
                fail_notification: true,
            });
            service.configure_reminder(
                true,
                Some(std::path::PathBuf::from("configured-sound")),
                100,
            );
            service
                .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
                .expect("configure");
            let start = service.now;
            let transition = service.timer.start(start, None);
            service
                .apply_transition(transition, 1)
                .expect("start transition");
            service
                .persist(Some("outbox-start"))
                .expect("persist start");

            service.apply_observation(start + 1, service.wall + 1);
            assert_eq!(
                *first_attempts.lock().expect("attempts"),
                vec![ReminderEffectKind::Notification, ReminderEffectKind::Sound]
            );
            let pending = service
                .repository
                .as_ref()
                .expect("repository")
                .due_reminder_effects(i64::MAX)
                .expect("pending");
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].kind, ReminderEffectKind::Notification);
        }

        let recovered_attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut restarted = Service::open(&path).expect("restart");
        restarted.reminder = Box::new(RecordingReminder {
            attempts: std::sync::Arc::clone(&recovered_attempts),
            fail_notification: false,
        });
        restarted.wall = restarted.wall.saturating_add(10);
        restarted.dispatch_pending_reminders();
        assert_eq!(
            *recovered_attempts.lock().expect("attempts"),
            vec![ReminderEffectKind::Notification]
        );
        assert!(
            restarted
                .repository
                .as_ref()
                .expect("repository")
                .due_reminder_effects(i64::MAX)
                .expect("pending")
                .is_empty()
        );
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn reminder_retries_are_time_bounded_and_visible_in_snapshot() {
        let path = database_path().with_extension("retry.sqlite3");
        let _ = std::fs::remove_file(&path);
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut service = Service::open(&path).expect("service");
        service.reminder = Box::new(RecordingReminder {
            attempts: std::sync::Arc::clone(&attempts),
            fail_notification: true,
        });
        service
            .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
            .expect("configure");
        let start = service.now;
        let transition = service.timer.start(start, None);
        service
            .apply_transition(transition, 1)
            .expect("start transition");
        service.persist(Some("retry-start")).expect("persist start");

        service.apply_observation(start + 1, service.wall + 1);
        assert_eq!(service.snapshot().reminder_delivery.retrying, 1);
        service.dispatch_pending_reminders();
        assert_eq!(attempts.lock().expect("attempts").len(), 1);

        service.wall = service.wall.saturating_add(20);
        service.dispatch_pending_reminders();
        assert_eq!(service.snapshot().reminder_delivery.retrying, 1);
        service.wall = service.wall.saturating_add(100);
        service.dispatch_pending_reminders();

        assert_eq!(attempts.lock().expect("attempts").len(), 3);
        assert_eq!(service.snapshot().reminder_delivery.retrying, 0);
        assert_eq!(service.snapshot().reminder_delivery.exhausted, 1);
        service.wall = service.wall.saturating_add(10_000);
        service.dispatch_pending_reminders();
        assert_eq!(attempts.lock().expect("attempts").len(), 3);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn stale_reminder_delivery_exhausts_before_the_attempt_limit() {
        let path = database_path().with_extension("retry-age.sqlite3");
        let _ = std::fs::remove_file(&path);
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut service = Service::open(&path).expect("service");
        service.reminder = Box::new(RecordingReminder {
            attempts: std::sync::Arc::clone(&attempts),
            fail_notification: true,
        });
        service
            .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
            .expect("configure");
        let start = service.now;
        let transition = service.timer.start(start, None);
        service
            .apply_transition(transition, 1)
            .expect("start transition");
        service
            .persist(Some("retry-age-start"))
            .expect("persist start");
        service.apply_observation(start + 1, service.wall + 1);

        service.wall = service.wall.saturating_add(MAX_REMINDER_AGE_SECONDS + 1);
        service.dispatch_pending_reminders();

        assert_eq!(attempts.lock().expect("attempts").len(), 2);
        assert_eq!(service.snapshot().reminder_delivery.exhausted, 1);
        std::fs::remove_file(path).expect("cleanup");
    }

    struct RecordingReminder {
        attempts: std::sync::Arc<std::sync::Mutex<Vec<ReminderEffectKind>>>,
        fail_notification: bool,
    }

    impl ReminderEffects for RecordingReminder {
        fn configure(&mut self, _sound: Option<std::path::PathBuf>, _volume_percent: u8) {}

        fn notify(&mut self) -> Result<(), String> {
            self.attempts
                .lock()
                .expect("attempts")
                .push(ReminderEffectKind::Notification);
            if self.fail_notification {
                Err("injected notification failure".into())
            } else {
                Ok(())
            }
        }

        fn play_sound(&mut self) -> Result<(), String> {
            self.attempts
                .lock()
                .expect("attempts")
                .push(ReminderEffectKind::Sound);
            Ok(())
        }
    }

    impl FailingRepository {
        fn write(&mut self) -> Result<(), String> {
            if self.successful_writes_remaining == 0 {
                Err("injected durable write failure".into())
            } else {
                self.successful_writes_remaining -= 1;
                Ok(())
            }
        }
    }

    #[test]
    fn pending_session_releases_its_task_for_deletion() {
        let mut service = Service::new();
        service.handle(request(
            Some("create-delete"),
            Command::TaskCreate {
                title: "Disposable".into(),
            },
        ));
        service.handle(request(
            Some("start-delete"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-delete"), Command::Stop));

        let response = service.handle(request(
            Some("delete-pending"),
            Command::TaskDelete { id: 1 },
        ));

        assert!(matches!(response, Response::Snapshot { .. }));
        assert!(service.snapshot().tasks.is_empty());
        assert_eq!(service.timer.current_task(), None);
    }

    #[test]
    fn running_session_keeps_its_task_when_deletion_is_requested() {
        let mut service = Service::new();
        service.handle(request(
            Some("create-protected"),
            Command::TaskCreate {
                title: "Protected".into(),
            },
        ));
        service.handle(request(
            Some("start-protected"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));

        let response = service.handle(request(
            Some("delete-running"),
            Command::TaskDelete { id: 1 },
        ));

        assert!(matches!(response, Response::Error { .. }));
        assert_eq!(service.snapshot().tasks.len(), 1);
        assert_eq!(service.timer.current_task(), Some(TaskId::new(1)));
    }

    #[test]
    fn snapshot_exposes_all_session_history() {
        let mut service = Service::new();
        for index in 0..8 {
            service.history.push(SessionRecord {
                id: 1,
                ended_at: index,
                kind: DomainKind::Focus,
                outcome: SessionOutcome::Stopped,
                planned_seconds: 1_500,
                actual_seconds: 1,
                task_id: None,
                task_title: Some(format!("Task {index}")),
            });
        }

        assert_eq!(service.snapshot().recent_history.len(), 8);
    }

    #[test]
    fn task_selection_rebinds_pending_and_confirmed_switch_stops_running_focus() {
        let mut service = Service::new();
        for (key, title) in [("create-one", "One"), ("create-two", "Two")] {
            service.handle(request(
                Some(key),
                Command::TaskCreate {
                    title: title.into(),
                },
            ));
        }
        service.handle(request(
            Some("select-one"),
            Command::TaskSelect {
                id: 1,
                stop_current: false,
            },
        ));
        assert_eq!(service.snapshot().current_task_id, Some(1));
        service.handle(request(
            Some("start-one"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(
            Some("switch-two"),
            Command::TaskSelect {
                id: 2,
                stop_current: true,
            },
        ));

        let snapshot = service.snapshot();
        assert_eq!(snapshot.state, "pending");
        assert_eq!(snapshot.current_task_id, Some(2));
        assert_eq!(snapshot.recent_history.len(), 1);
        assert_eq!(snapshot.recent_history[0].actual_seconds, 0);
        assert_eq!(
            snapshot.recent_history[0].task_title.as_deref(),
            Some("One")
        );
    }

    #[test]
    fn deleting_history_recalculates_task_and_daily_totals() {
        let mut service = Service::new();
        service.tasks.create("Tracked").expect("task");
        service.history.push(SessionRecord {
            id: 41,
            ended_at: service.wall,
            kind: DomainKind::Focus,
            outcome: SessionOutcome::Completed,
            planned_seconds: 60,
            actual_seconds: 60,
            task_id: Some(TaskId::new(1)),
            task_title: Some("Tracked".into()),
        });
        assert_eq!(service.snapshot().today.focus_seconds, 60);

        let response = service.handle(request(
            Some("delete-history"),
            Command::HistoryDelete { ids: vec![41] },
        ));

        assert!(matches!(response, Response::Snapshot { .. }));
        let snapshot = service.snapshot();
        assert!(snapshot.recent_history.is_empty());
        assert_eq!(snapshot.today.focus_seconds, 0);
        assert_eq!(snapshot.tasks[0].focus_seconds, 0);
    }

    #[test]
    fn local_day_boundaries_are_ordered_and_contain_the_observation() {
        let wall = LinuxClock.wall_seconds().expect("wall clock");
        let boundaries = local_day_boundaries(wall);
        assert!(boundaries.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(wall >= boundaries[6]);
        assert!(wall < boundaries[7]);
    }

    #[test]
    fn today_task_focus_excludes_older_history_and_sorts_descending() {
        let mut service = Service::new();
        let starts = local_day_boundaries(service.wall);
        for (id, ended_at, title, seconds) in [
            (1, starts[6] + 10, Some("Second"), 300),
            (2, starts[6] + 20, Some("First"), 900),
            (3, starts[5] + 20, Some("Older"), 3_600),
            (4, starts[6] + 30, None, 120),
        ] {
            service.history.push(SessionRecord {
                id,
                ended_at,
                kind: DomainKind::Focus,
                outcome: SessionOutcome::Completed,
                planned_seconds: seconds,
                actual_seconds: seconds,
                task_id: None,
                task_title: title.map(str::to_owned),
            });
        }

        let today = service.snapshot().today;
        assert_eq!(today.focus_seconds, 1_320);
        assert_eq!(
            today
                .task_focus
                .iter()
                .map(|item| (item.task_title.as_deref(), item.focus_seconds))
                .collect::<Vec<_>>(),
            [(Some("First"), 900), (Some("Second"), 300), (None, 120)]
        );
        assert!(today.seven_day_dates[6].starts_with("20"));
    }
}
