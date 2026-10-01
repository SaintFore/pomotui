use pomotui_domain::{
    CurrentSession, History, SessionDurations, SessionKind as DomainKind, SessionOutcome,
    SessionRecord, SessionState, TaskError, TaskId, TaskStatus, TaskStore, Timer, TimerState,
    Transition,
};
use pomotui_platform::{
    Clock, DesktopReminder, PendingReminderEffect, PlatformClock, RecoveryObservation,
    ReminderDeliveryCounts, ReminderEffectKind, ReminderPort, SqliteRepository,
    elapsed_during_recovery, read_sync_file, replace_sync_file,
};
use pomotui_protocol::{
    ActionChainSummary, ChainLinkSummary, Command, DurableHealth, DurableHealthState,
    EndedChainSummary, Handler, PendingReviewSummary, ProtocolError, RecentSessionSummary,
    ReminderDelivery, Request, Response, RewardMilestoneSummary, RewardUnlockSummary, SessionKind,
    Snapshot, SyncStatus, TaskFocusSummary, TaskSummary, TodaySummary,
};
use pomotui_sync::{
    ActivityProjection, Document as SyncDocument, EntityId, FORMAT_VERSION as SYNC_FORMAT_VERSION,
    MutationInstant, Record as SyncRecord, RecordId, RecordPayload, SessionKind as SyncSessionKind,
    SessionOutcome as SyncSessionOutcome, SessionReviewJudgment, SessionReviewProjection, SyncPlan,
    TaskProjection, TaskStatus as SyncTaskStatus, plan_sync,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

mod sync_worker;
pub use sync_worker::{SyncFileAdapter, SyncServiceHandler, SyncWorker, SyncWorkerTrigger};

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
    chain_links: Vec<ChainLinkState>,
    next_chain_entry_id: u64,
    void_task_id: Option<u64>,
    ended_chains: Vec<EndedChainState>,
    reward_milestones: Vec<RewardMilestoneState>,
    reward_unlocks: Vec<RewardUnlockState>,
    next_reward_milestone_id: u64,
    next_reward_unlock_id: u64,
    sync: SyncState,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct SyncState {
    path: Option<std::path::PathBuf>,
    records: Vec<SyncRecord>,
    task_entities: std::collections::BTreeMap<u64, EntityId>,
    #[serde(default)]
    session_entities: std::collections::BTreeMap<u64, EntityId>,
    #[serde(default, alias = "review_entries")]
    session_review_entries: std::collections::BTreeMap<EntityId, u64>,
    last_attempt: Option<i64>,
    last_success: Option<i64>,
    last_error: Option<String>,
    #[serde(default)]
    last_error_stage: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    file_record_count: Option<usize>,
    #[serde(skip)]
    in_progress: bool,
    #[serde(skip)]
    background: bool,
}

/// Immutable input captured by the synchronization worker while it briefly owns
/// the Timer Service state lock.
#[derive(Clone, Debug)]
pub struct SyncWork {
    pub path: std::path::PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PendingReviewState {
    session_id: u64,
    actual_seconds: u64,
    task_id: Option<u64>,
    task_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ChainLinkState {
    id: u64,
    session_id: u64,
    task_id: u64,
    task_title: String,
    actual_seconds: u64,
    reflection: Option<String>,
    chain_entry_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ChainBreakState {
    id: u64,
    session_id: u64,
    task_id: u64,
    task_title: String,
    actual_seconds: u64,
    reflection: String,
    chain_entry_title: Option<String>,
}

#[derive(Clone, Debug)]
struct SubmittedSessionReviewSync {
    entry_id: u64,
    session_id: u64,
    task_id: u64,
    task_title: String,
    actual_seconds: u64,
    judgment: SessionReviewJudgment,
    reflection: Option<String>,
    chain_entry_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EndedChainState {
    id: u64,
    links: Vec<ChainLinkState>,
    chain_break: ChainBreakState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RewardMilestoneState {
    id: u64,
    name: String,
    threshold: u64,
    budget: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RewardUnlockState {
    id: u64,
    milestone_id: u64,
    chain_id: u64,
    name: String,
    threshold: u64,
    budget: Option<u64>,
    state: String,
    claimed_at: Option<i64>,
}

impl Service {
    /// Creates the default v1 Timer Service state.
    ///
    /// # Panics
    ///
    /// Only panics if compile-time default durations become invalid.
    #[must_use]
    pub fn new() -> Self {
        let clock = PlatformClock::default();
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
            chain_links: Vec::new(),
            next_chain_entry_id: 1,
            void_task_id: None,
            ended_chains: Vec::new(),
            reward_milestones: Vec::new(),
            reward_unlocks: Vec::new(),
            next_reward_milestone_id: 1,
            next_reward_unlock_id: 1,
            sync: SyncState::default(),
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
        let created_void = service.ensure_void_task()?;
        service.applied_keys = keys;
        service.repository = Some(Box::new(repository));
        if !had_payload || created_void {
            service.persist(None)?;
        }
        service.observe_time();
        Ok(service)
    }

    /// Makes sync commands schedule work instead of performing file I/O inline.
    pub fn enable_background_sync(&mut self) {
        self.sync.background = true;
    }

    /// Starts a worker attempt and captures the file-independent input.
    ///
    /// # Errors
    ///
    /// Returns an error when synchronization is disabled.
    pub fn begin_sync_work(&mut self) -> Result<SyncWork, String> {
        let path = self
            .sync
            .path
            .clone()
            .ok_or_else(|| "synchronization is not enabled".to_owned())?;
        self.observe_time();
        self.sync.in_progress = true;
        self.sync.last_attempt = Some(self.wall);
        self.sync.last_error = None;
        self.sync.last_error_stage = None;
        self.sync.file_record_count = None;
        let _ = self.persist(None);
        Ok(SyncWork { path })
    }

    /// Captures the latest locally retained records for worker-side planning.
    ///
    /// # Errors
    ///
    /// Returns an error if the synchronization path changed during the attempt.
    pub fn sync_records_for(&self, path: &Path) -> Result<Vec<SyncRecord>, String> {
        if self.sync.path.as_deref() != Some(path) {
            return Err("synchronization configuration changed during the attempt".into());
        }
        Ok(self.sync.records.clone())
    }

    /// Applies validated incoming records and returns the latest retained union.
    /// The state is restored if its durable commit fails.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration changed, records conflict, projection
    /// fails, or the resulting state cannot be committed.
    pub fn apply_sync_plan(
        &mut self,
        path: &Path,
        plan: &SyncPlan,
        file_record_count: usize,
    ) -> Result<Vec<SyncRecord>, String> {
        if self.sync.path.as_deref() != Some(path) {
            return Err("synchronization configuration changed during the attempt".into());
        }
        if self.sync.records != plan.base_records() {
            return Err("local synchronization records changed during the attempt".into());
        }
        let old_sync = self.sync.clone();
        let old_tasks = self.tasks.clone();
        let old_history = self.history.clone();
        let old_next_event_id = self.next_event_id;
        let old_chain_links = self.chain_links.clone();
        let old_ended_chains = self.ended_chains.clone();
        let old_current_chain_id = self.current_chain_id;
        let old_current_chain_length = self.current_chain_length;
        let old_next_chain_entry_id = self.next_chain_entry_id;
        self.sync.records = plan.retained_records().to_vec();
        self.sync.file_record_count = Some(file_record_count);
        if let Err(error) = self
            .apply_task_projections(plan.task_projections())
            .and_then(|()| {
                self.apply_activity_projections(plan.activity_projections());
                self.apply_session_review_projection(plan.session_review_projection())?;
                self.set_clock_warning(plan.retained_records());
                self.persist(None)
            })
        {
            self.sync = old_sync;
            self.tasks = old_tasks;
            self.history = old_history;
            self.next_event_id = old_next_event_id;
            self.chain_links = old_chain_links;
            self.ended_chains = old_ended_chains;
            self.current_chain_id = old_current_chain_id;
            self.current_chain_length = old_current_chain_length;
            self.next_chain_entry_id = old_next_chain_entry_id;
            return Err(error);
        }
        Ok(self.sync.records.clone())
    }

    /// Records a fully completed import and atomic replacement.
    pub fn finish_sync_success(
        &mut self,
        path: &Path,
        file_record_count: usize,
        warning: Option<String>,
    ) {
        if self.sync.path.as_deref() != Some(path) {
            return;
        }
        self.observe_time();
        self.sync.in_progress = false;
        self.sync.file_record_count = Some(file_record_count);
        self.sync.last_success = Some(self.wall);
        self.sync.last_error = None;
        self.sync.last_error_stage = None;
        if warning.is_some() {
            self.sync.warning = warning;
        }
        let _ = self.persist(None);
    }

    /// Records a stage-specific sync failure without changing durable health.
    pub fn finish_sync_failure(&mut self, path: &Path, stage: &str, error: String) {
        if self.sync.path.as_deref() != Some(path) {
            return;
        }
        self.sync.in_progress = false;
        self.sync.last_error_stage = Some(stage.into());
        self.sync.last_error = Some(error);
        let _ = self.persist(None);
    }

    fn ensure_void_task(&mut self) -> Result<bool, String> {
        const VOID_TASK_ID: u64 = u64::MAX;
        if self.void_task_id.is_some() {
            return Ok(false);
        }
        let mut tasks = self
            .tasks
            .all()
            .iter()
            .map(|task| (task.id(), task.title().to_owned(), task.status()))
            .collect::<Vec<_>>();
        tasks.push((TaskId::new(VOID_TASK_ID), "Void".into(), TaskStatus::Open));
        self.tasks =
            TaskStore::restore(tasks, self.tasks.next_id()).map_err(|error| error.to_string())?;
        self.void_task_id = Some(VOID_TASK_ID);
        Ok(true)
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

    #[allow(clippy::too_many_lines)]
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
                    is_void: review.task_id == self.void_task_id,
                }),
            recent_chain_links: self
                .chain_links
                .iter()
                .map(|link| ChainLinkSummary {
                    id: link.id,
                    task_title: link.task_title.clone(),
                    actual_seconds: link.actual_seconds,
                    reflection: link.reflection.clone(),
                    chain_entry_title: link.chain_entry_title.clone(),
                })
                .collect(),
            recent_ended_chains: self
                .ended_chains
                .iter()
                .rev()
                .take(5)
                .map(|chain| EndedChainSummary {
                    id: chain.id,
                    length: u64::try_from(chain.links.len()).unwrap_or(u64::MAX),
                    links: chain
                        .links
                        .iter()
                        .map(|link| ChainLinkSummary {
                            id: link.id,
                            task_title: link.task_title.clone(),
                            actual_seconds: link.actual_seconds,
                            reflection: link.reflection.clone(),
                            chain_entry_title: link.chain_entry_title.clone(),
                        })
                        .collect(),
                    break_id: chain.chain_break.id,
                    break_task_title: chain.chain_break.task_title.clone(),
                    break_actual_seconds: chain.chain_break.actual_seconds,
                    break_reflection: chain.chain_break.reflection.clone(),
                    break_chain_entry_title: chain.chain_break.chain_entry_title.clone(),
                    rewards: self
                        .reward_unlocks
                        .iter()
                        .filter(|reward| reward.chain_id == chain.id)
                        .map(|reward| RewardUnlockSummary {
                            id: reward.id,
                            name: reward.name.clone(),
                            threshold: reward.threshold,
                            budget: reward.budget,
                            state: reward.state.clone(),
                        })
                        .collect(),
                })
                .collect(),
            next_reward: self
                .reward_milestones
                .iter()
                .filter(|milestone| milestone.threshold > self.current_chain_length)
                .min_by_key(|milestone| milestone.threshold)
                .map(|milestone| RewardMilestoneSummary {
                    id: milestone.id,
                    name: milestone.name.clone(),
                    threshold: milestone.threshold,
                    budget: milestone.budget,
                }),
            reward_milestones: {
                let mut milestones = self
                    .reward_milestones
                    .iter()
                    .map(|milestone| RewardMilestoneSummary {
                        id: milestone.id,
                        name: milestone.name.clone(),
                        threshold: milestone.threshold,
                        budget: milestone.budget,
                    })
                    .collect::<Vec<_>>();
                milestones.sort_by_key(|milestone| (milestone.threshold, milestone.id));
                milestones
            },
            current_chain_rewards: self
                .reward_unlocks
                .iter()
                .filter(|unlock| unlock.chain_id == self.current_chain_id)
                .map(|unlock| RewardUnlockSummary {
                    id: unlock.id,
                    name: unlock.name.clone(),
                    threshold: unlock.threshold,
                    budget: unlock.budget,
                    state: unlock.state.clone(),
                })
                .collect(),
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
        let clock = PlatformClock::default();
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
            self.record_ended_session(self.next_event_id);
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

    fn review_success(
        &mut self,
        task_id: Option<u64>,
        use_void: bool,
        chain_entry_title: Option<String>,
        reflection: Option<String>,
    ) -> Result<(), String> {
        let chain_entry_title = chain_entry_title
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if use_void && chain_entry_title.is_none() {
            return Err("Void Session Review requires a Chain Entry Title".into());
        }
        if !use_void && chain_entry_title.is_some() {
            return Err("Only Void entries can have a Chain Entry Title".into());
        }
        let Some(mut review) = self.pending_review.take() else {
            return Err("No Pending Review is available".into());
        };
        if review.task_id.is_none() {
            let assigned_id = if use_void {
                if let Some(id) = self.void_task_id {
                    TaskId::new(id)
                } else {
                    let id = self
                        .tasks
                        .create("Void")
                        .map_err(|error| error.to_string())?;
                    self.void_task_id = Some(id.get());
                    id
                }
            } else {
                TaskId::new(
                    task_id.ok_or("Pending Review must be assigned to a Task before submission")?,
                )
            };
            let task_title = self
                .tasks
                .get(assigned_id)
                .map_err(|error| error.to_string())?
                .title()
                .to_owned();
            self.history
                .attribute(review.session_id, assigned_id, &task_title)
                .map_err(str::to_owned)?;
            review.task_id = Some(assigned_id.get());
            review.task_title = Some(task_title);
        }
        let is_void = review.task_id == self.void_task_id;
        if is_void && chain_entry_title.is_none() {
            self.pending_review = Some(review);
            return Err("Void Session Review requires a Chain Entry Title".into());
        }
        let task_id = review.task_id.expect("review attribution validated");
        let task_title = review.task_title.expect("review Task snapshot validated");
        let review_reflection = reflection.filter(|value| !value.trim().is_empty());
        self.chain_links.push(ChainLinkState {
            id: self.next_chain_entry_id,
            session_id: review.session_id,
            task_id,
            task_title: task_title.clone(),
            actual_seconds: review.actual_seconds,
            reflection: review_reflection.clone(),
            chain_entry_title: chain_entry_title.clone(),
        });
        self.record_submitted_session_review(SubmittedSessionReviewSync {
            entry_id: self.next_chain_entry_id,
            session_id: review.session_id,
            task_id,
            task_title,
            actual_seconds: review.actual_seconds,
            judgment: SessionReviewJudgment::Successful,
            reflection: review_reflection,
            chain_entry_title,
        })?;
        self.next_chain_entry_id = self.next_chain_entry_id.saturating_add(1);
        self.current_chain_length = self.current_chain_length.saturating_add(1);
        self.unlock_eligible_rewards();
        Ok(())
    }

    fn unlock_eligible_rewards(&mut self) {
        for milestone in &self.reward_milestones {
            let exists = self.reward_unlocks.iter().any(|unlock| {
                unlock.chain_id == self.current_chain_id && unlock.milestone_id == milestone.id
            });
            if milestone.threshold <= self.current_chain_length && !exists {
                self.reward_unlocks.push(RewardUnlockState {
                    id: self.next_reward_unlock_id,
                    milestone_id: milestone.id,
                    chain_id: self.current_chain_id,
                    name: milestone.name.clone(),
                    threshold: milestone.threshold,
                    budget: milestone.budget,
                    state: "unlocked".into(),
                    claimed_at: None,
                });
                self.next_reward_unlock_id = self.next_reward_unlock_id.saturating_add(1);
            }
        }
    }

    fn review_failure(
        &mut self,
        reflection: &str,
        task_id: Option<u64>,
        use_void: bool,
        chain_entry_title: Option<String>,
    ) -> Result<(), String> {
        let reflection = reflection.trim().to_owned();
        if reflection.is_empty() {
            return Err("Failed Session Review requires a Reflection".into());
        }
        let Some(review_state) = self.pending_review.as_ref() else {
            return Err("No Pending Review is available".into());
        };
        if review_state.task_id.is_none() && task_id.is_some() == use_void {
            return Err("Choose exactly one regular Task or the Void Task".into());
        }
        let chain_entry_title = chain_entry_title
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if use_void && chain_entry_title.is_none() {
            return Err("Void Session Review requires a Chain Entry Title".into());
        }
        let mut review = self.pending_review.take().expect("review checked above");
        if review.task_id.is_none() {
            let assigned_id = if use_void {
                if let Some(id) = self.void_task_id {
                    TaskId::new(id)
                } else {
                    let id = self
                        .tasks
                        .create("Void")
                        .map_err(|error| error.to_string())?;
                    self.void_task_id = Some(id.get());
                    id
                }
            } else {
                TaskId::new(task_id.expect("regular Task choice validated"))
            };
            let title = self
                .tasks
                .get(assigned_id)
                .map_err(|error| error.to_string())?
                .title()
                .to_owned();
            self.history
                .attribute(review.session_id, assigned_id, &title)
                .map_err(str::to_owned)?;
            review.task_id = Some(assigned_id.get());
            review.task_title = Some(title);
        }
        let is_void = review.task_id == self.void_task_id;
        if is_void && chain_entry_title.is_none() {
            self.pending_review = Some(review);
            return Err("Void Session Review requires a Chain Entry Title".into());
        }
        if !is_void && chain_entry_title.is_some() {
            self.pending_review = Some(review);
            return Err("Only Void entries can have a Chain Entry Title".into());
        }
        let review_session_id = review.session_id;
        let review_task_id = review.task_id.expect("review attribution validated");
        let review_task_title = review.task_title.expect("review Task snapshot validated");
        let review_actual_seconds = review.actual_seconds;
        let chain_break = ChainBreakState {
            id: self.next_chain_entry_id,
            session_id: review_session_id,
            task_id: review_task_id,
            task_title: review_task_title.clone(),
            actual_seconds: review_actual_seconds,
            reflection: reflection.clone(),
            chain_entry_title: chain_entry_title.clone(),
        };
        self.record_submitted_session_review(SubmittedSessionReviewSync {
            entry_id: self.next_chain_entry_id,
            session_id: review_session_id,
            task_id: review_task_id,
            task_title: review_task_title,
            actual_seconds: review_actual_seconds,
            judgment: SessionReviewJudgment::Failed,
            reflection: Some(reflection),
            chain_entry_title: chain_entry_title.clone(),
        })?;
        self.next_chain_entry_id = self.next_chain_entry_id.saturating_add(1);
        self.ended_chains.push(EndedChainState {
            id: self.current_chain_id,
            links: std::mem::take(&mut self.chain_links),
            chain_break,
        });
        for unlock in &mut self.reward_unlocks {
            if unlock.chain_id == self.current_chain_id && unlock.state == "unlocked" {
                unlock.state = "unavailable".into();
            }
        }
        self.current_chain_id = self.current_chain_id.saturating_add(1);
        self.current_chain_length = 0;
        Ok(())
    }

    fn edit_chain_entry(
        &mut self,
        id: u64,
        reflection: Option<String>,
        chain_entry_title: Option<String>,
    ) -> Result<(), String> {
        let reflection = reflection
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let chain_entry_title = chain_entry_title
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if let Some(link) = self.chain_links.iter_mut().find(|link| link.id == id) {
            if let Some(title) = chain_entry_title {
                link.chain_entry_title = Some(title);
            }
            if reflection.is_some() {
                link.reflection = reflection;
            }
            return Ok(());
        }
        for chain in &mut self.ended_chains {
            if let Some(link) = chain.links.iter_mut().find(|link| link.id == id) {
                if let Some(title) = chain_entry_title {
                    link.chain_entry_title = Some(title);
                }
                if reflection.is_some() {
                    link.reflection = reflection;
                }
                return Ok(());
            }
            if chain.chain_break.id == id {
                if let Some(title) = chain_entry_title {
                    chain.chain_break.chain_entry_title = Some(title);
                }
                if let Some(reflection) = reflection {
                    chain.chain_break.reflection = reflection;
                }
                return Ok(());
            }
        }
        Err(format!("Chain entry {id} does not exist"))
    }

    fn persist(&mut self, key: Option<&str>) -> Result<(), String> {
        let payload = PersistedService::encode(self)?;
        let Some(repository) = &mut self.repository else {
            if let Some(key) = key {
                self.applied_keys.insert(key.to_owned());
            }
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

    fn record_task_creation(&mut self, task_id: u64, title: &str) {
        let entity_id = EntityId::random();
        self.sync.task_entities.insert(task_id, entity_id.clone());
        self.sync.records.push(SyncRecord::new(
            RecordId::random(),
            entity_id,
            self.next_sync_mutation_time(),
            RecordPayload::TaskVersion {
                title: title.into(),
                status: SyncTaskStatus::Open,
            },
        ));
        self.sync.records.sort();
    }

    fn next_sync_mutation_time(&self) -> MutationInstant {
        let millis = self
            .sync
            .records
            .iter()
            .map(|record| record.mutation_time.as_millis())
            .max()
            .unwrap_or(i64::MIN)
            .saturating_add(1)
            .max(self.wall.saturating_mul(1_000));
        MutationInstant::from_millis(millis).expect("service wall time is a valid UTC instant")
    }

    fn record_task_version(&mut self, task_id: u64) -> Result<(), String> {
        let task = self
            .tasks
            .get(TaskId::new(task_id))
            .map_err(|error| error.to_string())?;
        let title = task.title().to_owned();
        let status = match task.status() {
            TaskStatus::Open => SyncTaskStatus::Open,
            TaskStatus::Completed => SyncTaskStatus::Completed,
        };
        let entity_id = self
            .sync
            .task_entities
            .get(&task_id)
            .cloned()
            .ok_or_else(|| format!("Task {task_id} has no global identity"))?;
        self.sync.records.push(SyncRecord::new(
            RecordId::random(),
            entity_id,
            self.next_sync_mutation_time(),
            RecordPayload::TaskVersion { title, status },
        ));
        self.sync.records.sort();
        Ok(())
    }

    fn record_task_deletion(&mut self, task_id: u64) -> Result<(), String> {
        let entity_id = self
            .sync
            .task_entities
            .get(&task_id)
            .cloned()
            .ok_or_else(|| format!("Task {task_id} has no global identity"))?;
        self.sync.records.push(SyncRecord::new(
            RecordId::random(),
            entity_id,
            self.next_sync_mutation_time(),
            RecordPayload::TaskDeleted,
        ));
        self.sync.records.sort();
        Ok(())
    }

    fn record_ended_session(&mut self, session_id: u64) {
        let Some(record) = self
            .history
            .records()
            .iter()
            .find(|record| record.id == session_id)
            .cloned()
        else {
            return;
        };
        let entity_id = EntityId::random();
        self.sync
            .session_entities
            .insert(session_id, entity_id.clone());
        let task_entity_id = record
            .task_id
            .and_then(|id| self.sync.task_entities.get(&id.get()).cloned());
        self.sync.records.push(SyncRecord::new(
            RecordId::random(),
            entity_id,
            self.next_sync_mutation_time(),
            RecordPayload::SessionEnded {
                ended_at: record.ended_at,
                kind: sync_kind(record.kind),
                outcome: sync_outcome(record.outcome),
                planned_seconds: record.planned_seconds,
                actual_seconds: record.actual_seconds,
                task_entity_id,
                task_title: record.task_title.clone(),
            },
        ));
        self.sync.records.sort();
    }

    fn record_submitted_session_review(
        &mut self,
        review: SubmittedSessionReviewSync,
    ) -> Result<(), String> {
        let session_entity_id = self
            .sync
            .session_entities
            .get(&review.session_id)
            .cloned()
            .ok_or_else(|| "submitted Session Review source has no global identity".to_owned())?;
        if !self.sync.task_entities.contains_key(&review.task_id) {
            self.record_task_creation(review.task_id, &review.task_title);
        }
        let task_entity_id = self.sync.task_entities[&review.task_id].clone();
        let review_entity_id = EntityId::random();
        self.sync
            .session_review_entries
            .insert(review_entity_id.clone(), review.entry_id);
        self.sync.records.push(SyncRecord::new(
            RecordId::random(),
            review_entity_id,
            self.next_sync_mutation_time(),
            RecordPayload::SessionReviewed {
                session_entity_id,
                judgment: review.judgment,
                task_entity_id,
                task_title: review.task_title,
                actual_seconds: review.actual_seconds,
                reflection: review.reflection,
                chain_entry_title: review.chain_entry_title,
            },
        ));
        self.sync.records.sort();
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn backfill_sync_records(&mut self) -> Result<(), String> {
        let missing_tasks = self
            .tasks
            .all()
            .iter()
            .filter(|task| {
                Some(task.id().get()) != self.void_task_id
                    && !self.sync.task_entities.contains_key(&task.id().get())
            })
            .map(|task| (task.id().get(), task.title().to_owned(), task.status()))
            .collect::<Vec<_>>();
        for (task_id, title, status) in missing_tasks {
            self.record_task_creation(task_id, &title);
            if status == TaskStatus::Completed {
                self.record_task_version(task_id)?;
            }
        }
        let deleted_task_snapshots = self
            .history
            .records()
            .iter()
            .filter_map(|record| Some((record.task_id?.get(), record.task_title.clone()?)))
            .filter(|(task_id, _)| !self.sync.task_entities.contains_key(task_id))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (task_id, title) in deleted_task_snapshots {
            self.record_task_creation(task_id, &title);
            self.record_task_deletion(task_id)?;
        }
        let missing_sessions = self
            .history
            .records()
            .iter()
            .filter(|record| !self.sync.session_entities.contains_key(&record.id))
            .map(|record| record.id)
            .collect::<Vec<_>>();
        for session_id in missing_sessions {
            self.record_ended_session(session_id);
        }
        let known_entries = self
            .sync
            .session_review_entries
            .values()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut missing_reviews = self
            .ended_chains
            .iter()
            .flat_map(|chain| {
                chain
                    .links
                    .iter()
                    .map(|entry| SubmittedSessionReviewSync {
                        entry_id: entry.id,
                        session_id: entry.session_id,
                        task_id: entry.task_id,
                        task_title: entry.task_title.clone(),
                        actual_seconds: entry.actual_seconds,
                        judgment: SessionReviewJudgment::Successful,
                        reflection: entry.reflection.clone(),
                        chain_entry_title: entry.chain_entry_title.clone(),
                    })
                    .chain(std::iter::once(SubmittedSessionReviewSync {
                        entry_id: chain.chain_break.id,
                        session_id: chain.chain_break.session_id,
                        task_id: chain.chain_break.task_id,
                        task_title: chain.chain_break.task_title.clone(),
                        actual_seconds: chain.chain_break.actual_seconds,
                        judgment: SessionReviewJudgment::Failed,
                        reflection: Some(chain.chain_break.reflection.clone()),
                        chain_entry_title: chain.chain_break.chain_entry_title.clone(),
                    }))
                    .collect::<Vec<_>>()
            })
            .chain(
                self.chain_links
                    .iter()
                    .map(|entry| SubmittedSessionReviewSync {
                        entry_id: entry.id,
                        session_id: entry.session_id,
                        task_id: entry.task_id,
                        task_title: entry.task_title.clone(),
                        actual_seconds: entry.actual_seconds,
                        judgment: SessionReviewJudgment::Successful,
                        reflection: entry.reflection.clone(),
                        chain_entry_title: entry.chain_entry_title.clone(),
                    }),
            )
            .filter(|review| !known_entries.contains(&review.entry_id))
            .collect::<Vec<_>>();
        missing_reviews.sort_by_key(|review| review.entry_id);
        for review in missing_reviews {
            self.record_submitted_session_review(review)?;
        }
        Ok(())
    }

    fn finish_task_mutation(&mut self, key: Option<&str>) -> Response {
        match self.persist(key) {
            Ok(()) => {
                if self.sync.path.is_some()
                    && !self.sync.background
                    && let Err(error) = self.merge_sync_file()
                {
                    self.sync.last_error = Some(error);
                    let _ = self.persist(None);
                }
                Response::Snapshot {
                    snapshot: self.snapshot(),
                }
            }
            Err(error) => self.durable_rejected(error),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn merge_sync_file(&mut self) -> Result<usize, String> {
        let path = self
            .sync
            .path
            .clone()
            .ok_or_else(|| "synchronization is not enabled".to_owned())?;
        self.sync.last_attempt = Some(self.wall);
        let source = read_sync_file(&path)?.ok_or_else(|| {
            "sync file does not exist; use `pomotui sync rebuild` to create it from local records"
                .to_owned()
        })?;
        let incoming = SyncDocument::from_json(&source)?.into_records();
        self.sync.file_record_count = Some(incoming.len());
        let plan = plan_sync(&self.sync.records, &incoming)?;
        self.sync.records = plan.retained_records().to_vec();
        self.apply_task_projections(plan.task_projections())?;
        self.apply_activity_projections(plan.activity_projections());
        self.apply_session_review_projection(plan.session_review_projection())?;
        self.set_clock_warning(plan.retained_records());
        self.persist(None)?;
        let serialized = SyncDocument::new(&self.sync.records)?.to_json()?;
        replace_sync_file(&path, &serialized)?;
        self.sync.file_record_count = Some(self.sync.records.len());
        self.sync.last_success = Some(self.wall);
        self.sync.last_error = None;
        self.sync.last_error_stage = None;
        self.persist(None)?;
        Ok(self.sync.records.len())
    }

    fn rebuild_sync_file(&mut self) -> Result<usize, String> {
        let path = self
            .sync
            .path
            .clone()
            .ok_or_else(|| "synchronization is not enabled".to_owned())?;
        self.sync.last_attempt = Some(self.wall);
        let serialized = SyncDocument::new(&self.sync.records)?.to_json()?;
        replace_sync_file(&path, &serialized)?;
        self.sync.file_record_count = Some(self.sync.records.len());
        self.sync.last_success = Some(self.wall);
        self.sync.last_error = None;
        self.sync.last_error_stage = None;
        self.sync.warning = Some(
            "rebuilt from locally known records; unseen remote records cannot be recovered".into(),
        );
        self.persist(None)?;
        Ok(self.sync.records.len())
    }

    fn apply_task_projections(&mut self, projections: &[TaskProjection]) -> Result<(), String> {
        for projection in projections.iter().cloned() {
            let (entity_id, title, status) = match projection {
                TaskProjection::Version {
                    entity_id,
                    title,
                    status,
                    ..
                } => (entity_id, title, Some(status)),
                TaskProjection::Deleted {
                    entity_id,
                    last_title: Some(title),
                } => (entity_id, title, None),
                TaskProjection::Deleted {
                    entity_id: _,
                    last_title: None,
                } => continue,
            };
            let local_id = self
                .sync
                .task_entities
                .iter()
                .find_map(|(local_id, mapped)| (mapped == &entity_id).then_some(*local_id));
            let task_id = if let Some(local_id) = local_id {
                TaskId::new(local_id)
            } else {
                let task_id = self
                    .tasks
                    .create(&title)
                    .map_err(|error| error.to_string())?;
                self.sync
                    .task_entities
                    .insert(task_id.get(), entity_id.clone());
                task_id
            };
            if self.tasks.get(task_id).is_ok() {
                self.tasks
                    .rename(task_id, &title)
                    .map_err(|error| error.to_string())?;
                match status {
                    Some(SyncTaskStatus::Open) | None => self.tasks.reopen(task_id),
                    Some(SyncTaskStatus::Completed) => self.tasks.complete(task_id),
                }
                .map_err(|error| error.to_string())?;
            }
        }
        self.apply_deferred_task_deletions()?;
        Ok(())
    }

    fn apply_activity_projections(&mut self, projections: &[ActivityProjection]) {
        for projection in projections {
            match projection {
                ActivityProjection::Deleted { entity_id } => {
                    if let Some(local_id) = self
                        .sync
                        .session_entities
                        .iter()
                        .find_map(|(local_id, mapped)| (mapped == entity_id).then_some(*local_id))
                    {
                        self.history.delete(&[local_id]);
                    }
                }
                ActivityProjection::Session {
                    entity_id,
                    ended_at,
                    kind,
                    outcome,
                    planned_seconds,
                    actual_seconds,
                    task_entity_id,
                    task_title,
                } => {
                    if self
                        .sync
                        .session_entities
                        .values()
                        .any(|mapped| mapped == entity_id)
                    {
                        continue;
                    }
                    let task_id = task_entity_id.as_ref().map(|global_id| {
                        self.sync
                            .task_entities
                            .iter()
                            .find_map(|(local_id, mapped)| {
                                (mapped == global_id).then_some(TaskId::new(*local_id))
                            })
                            .expect("validated SyncPlan applies Task projections before Sessions")
                    });
                    let local_id = self.next_event_id;
                    self.next_event_id = self.next_event_id.saturating_add(1);
                    self.history.push(SessionRecord {
                        id: local_id,
                        ended_at: *ended_at,
                        kind: domain_sync_kind(*kind),
                        outcome: domain_sync_outcome(*outcome),
                        planned_seconds: *planned_seconds,
                        actual_seconds: *actual_seconds,
                        task_id,
                        task_title: task_title.clone(),
                    });
                    self.sync
                        .session_entities
                        .insert(local_id, entity_id.clone());
                }
            }
        }
        let session_entities = &self.sync.session_entities;
        let mut records = self.history.records().to_vec();
        records.sort_by(|left, right| {
            left.ended_at.cmp(&right.ended_at).then_with(|| {
                session_entities
                    .get(&left.id)
                    .cmp(&session_entities.get(&right.id))
            })
        });
        self.history = History::restore(records);
    }

    fn apply_session_review_projection(
        &mut self,
        projection: &SessionReviewProjection,
    ) -> Result<(), String> {
        let mut next_entry_id = self.next_chain_entry_id;
        let mut entry_id =
            |entity_id: &EntityId, mappings: &mut std::collections::BTreeMap<EntityId, u64>| {
                if let Some(id) = mappings.get(entity_id) {
                    *id
                } else {
                    let id = next_entry_id;
                    next_entry_id = next_entry_id.saturating_add(1);
                    mappings.insert(entity_id.clone(), id);
                    id
                }
            };
        let mut build_link =
            |review: &pomotui_sync::ProjectedSessionReview| -> Result<ChainLinkState, String> {
                let session_id = self
                    .sync
                    .session_entities
                    .iter()
                    .find_map(|(local, global)| {
                        (global == &review.session_entity_id).then_some(*local)
                    })
                    .ok_or_else(|| {
                        "projected Session Review source Session is not local".to_owned()
                    })?;
                let task_id = self
                    .sync
                    .task_entities
                    .iter()
                    .find_map(|(local, global)| {
                        (global == &review.task_entity_id).then_some(*local)
                    })
                    .ok_or_else(|| "projected Session Review Task is not local".to_owned())?;
                if self
                    .history
                    .records()
                    .iter()
                    .find(|record| record.id == session_id)
                    .is_some_and(|record| record.task_id.is_none())
                {
                    self.history
                        .attribute(session_id, TaskId::new(task_id), &review.task_title)
                        .map_err(str::to_owned)?;
                }
                Ok(ChainLinkState {
                    id: entry_id(
                        &review.review_entity_id,
                        &mut self.sync.session_review_entries,
                    ),
                    session_id,
                    task_id,
                    task_title: review.task_title.clone(),
                    actual_seconds: review.actual_seconds,
                    reflection: review.reflection.clone(),
                    chain_entry_title: review.chain_entry_title.clone(),
                })
            };
        let mut ended_chains = Vec::new();
        for (index, chain) in projection.ended_chains.iter().enumerate() {
            let links = chain
                .links
                .iter()
                .map(&mut build_link)
                .collect::<Result<Vec<_>, _>>()?;
            let break_review = chain
                .chain_break
                .as_ref()
                .ok_or_else(|| "ended projected chain has no Chain Break".to_owned())?;
            let break_link = build_link(break_review)?;
            ended_chains.push(EndedChainState {
                id: index as u64 + 1,
                links,
                chain_break: ChainBreakState {
                    id: break_link.id,
                    session_id: break_link.session_id,
                    task_id: break_link.task_id,
                    task_title: break_link.task_title,
                    actual_seconds: break_link.actual_seconds,
                    reflection: break_review.reflection.clone().unwrap_or_default(),
                    chain_entry_title: break_link.chain_entry_title,
                },
            });
        }
        let chain_links = projection
            .current_chain
            .links
            .iter()
            .map(&mut build_link)
            .collect::<Result<Vec<_>, _>>()?;
        self.ended_chains = ended_chains;
        self.chain_links = chain_links;
        self.current_chain_id = projection.ended_chains.len() as u64 + 1;
        self.current_chain_length = self.chain_links.len() as u64;
        self.next_chain_entry_id = next_entry_id;
        Ok(())
    }

    fn set_clock_warning(&mut self, records: &[SyncRecord]) {
        const IMPLAUSIBLE_SKEW_SECONDS: i64 = 366 * 24 * 60 * 60;
        let implausible = records.iter().any(|record| match record.payload {
            RecordPayload::SessionEnded { ended_at, .. } => {
                ended_at.abs_diff(self.wall) > IMPLAUSIBLE_SKEW_SECONDS as u64
            }
            _ => false,
        });
        self.sync.warning = implausible.then(|| {
            "synchronized activity contains a timestamp more than one year from local time; records were retained and projected".into()
        });
    }

    fn apply_deferred_task_deletions(&mut self) -> Result<(), String> {
        let deleted_entities = self
            .sync
            .records
            .iter()
            .filter(|record| matches!(record.payload, RecordPayload::TaskDeleted))
            .map(|record| &record.entity_id)
            .collect::<std::collections::HashSet<_>>();
        let local_ids = self
            .sync
            .task_entities
            .iter()
            .filter_map(|(local_id, entity_id)| {
                deleted_entities.contains(entity_id).then_some(*local_id)
            })
            .collect::<Vec<_>>();
        for local_id in local_ids {
            let task_id = TaskId::new(local_id);
            if self.tasks.get(task_id).is_ok() && self.timer.current_task() != Some(task_id) {
                self.tasks
                    .delete(task_id, self.timer.current_task())
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    fn sync_status(&self) -> serde_json::Value {
        serde_json::to_value(SyncStatus {
            stability: "experimental".into(),
            capabilities: vec!["task_lifecycle".into(), "session_history".into()],
            enabled: self.sync.path.is_some(),
            path: self.sync.path.clone(),
            format_version: SYNC_FORMAT_VERSION,
            in_progress: self.sync.in_progress,
            last_attempt: self.sync.last_attempt,
            last_success: self.sync.last_success,
            last_error: self.sync.last_error.clone(),
            last_error_stage: self.sync.last_error_stage.clone(),
            warning: self.sync.warning.clone(),
            local_record_count: self.sync.records.len(),
            file_record_count: self.sync.file_record_count,
        })
        .expect("SyncStatus is serializable")
    }

    fn merge_sync_response(&mut self) -> Response {
        match self.merge_sync_file() {
            Ok(_) => Response::Data {
                value: self.sync_status(),
            },
            Err(error) => {
                self.sync.last_error = Some(error.clone());
                self.sync.last_error_stage = Some("synchronize".into());
                let _ = self.persist(None);
                Self::rejected(error)
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
                        .and_then(|()| self.apply_deferred_task_deletions())
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
                    Err(pomotui_domain::TaskError::TitleNotFound(_)) => {
                        self.tasks.create(&title).inspect(|id| {
                            self.record_task_creation(id.get(), &title);
                        })
                    }
                    Err(error) => Err(error),
                };
                match task_id {
                    Ok(task_id) => {
                        let planned = self.timer.planned_seconds();
                        let transition = self.timer.start(self.now, Some(task_id));
                        self.apply_transition(transition, planned)
                            .and_then(|()| self.apply_deferred_task_deletions())
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
            Command::StopReview => {
                let planned = self.timer.planned_seconds();
                let transition = self.timer.stop(self.now);
                self.apply_transition(transition, planned).and_then(|()| {
                    let Some(record) = self.history.records().last() else {
                        return Err("Stopped Session was not recorded".into());
                    };
                    if record.kind != DomainKind::Focus {
                        return Err("Only a stopped Focus Session can enter Session Review".into());
                    }
                    self.pending_review = Some(PendingReviewState {
                        session_id: record.id,
                        actual_seconds: record.actual_seconds,
                        task_id: record.task_id.map(TaskId::get),
                        task_title: record.task_title.clone(),
                    });
                    Ok(())
                })
            }
            Command::Skip => {
                let planned = self.timer.planned_seconds();
                let transition = self.timer.skip();
                self.apply_transition(transition, planned)
            }
            Command::TaskCreate { title } => {
                return match self.tasks.create(title.clone()) {
                    Ok(id) => {
                        self.record_task_creation(id.get(), &title);
                        match self.persist(mutation_key.as_deref()) {
                            Ok(()) => {
                                if self.sync.path.is_some()
                                    && !self.sync.background
                                    && let Err(error) = self.merge_sync_file()
                                {
                                    self.sync.last_error = Some(error);
                                    let _ = self.persist(None);
                                }
                                Response::Data {
                                    value: serde_json::json!({ "id": id.get() }),
                                }
                            }
                            Err(error) => self.durable_rejected(error),
                        }
                    }
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
            Command::SyncEnable { path } => {
                if let Err(error) = self.backfill_sync_records() {
                    return Self::rejected(error);
                }
                self.sync.path = Some(path);
                self.sync.warning = None;
                if let Err(error) = self.persist(mutation_key.as_deref()) {
                    return self.durable_rejected(error);
                }
                if self.sync.background {
                    return Response::Data {
                        value: self.sync_status(),
                    };
                }
                return self.merge_sync_response();
            }
            Command::SyncDisable => {
                self.sync.path = None;
                self.sync.in_progress = false;
                self.sync.warning = None;
                return match self.persist(mutation_key.as_deref()) {
                    Ok(()) => Response::Data {
                        value: self.sync_status(),
                    },
                    Err(error) => self.durable_rejected(error),
                };
            }
            Command::SyncNow => {
                if self.sync.path.is_none() {
                    return Self::rejected("synchronization is not enabled");
                }
                if self.sync.background {
                    return Response::Data {
                        value: self.sync_status(),
                    };
                }
                return self.merge_sync_response();
            }
            Command::SyncRebuild => {
                if self.sync.path.is_none() {
                    return Self::rejected("synchronization is not enabled");
                }
                if self.sync.background {
                    return Response::Data {
                        value: self.sync_status(),
                    };
                }
                return match self.rebuild_sync_file() {
                    Ok(_) => Response::Data {
                        value: self.sync_status(),
                    },
                    Err(error) => {
                        self.sync.last_error = Some(error.clone());
                        self.sync.last_error_stage = Some("rebuild".into());
                        let _ = self.persist(None);
                        Self::rejected(error)
                    }
                };
            }
            Command::SyncStatus => {
                return Response::Data {
                    value: self.sync_status(),
                };
            }
            Command::TaskRename { id, title } => {
                if self.void_task_id == Some(id) {
                    return Self::rejected("Void Task cannot be renamed");
                }
                return match self.tasks.rename(pomotui_domain::TaskId::new(id), title) {
                    Ok(()) => match self.record_task_version(id) {
                        Ok(()) => self.finish_task_mutation(mutation_key.as_deref()),
                        Err(error) => Self::rejected(error),
                    },
                    Err(error) => Self::task_rejected(error),
                };
            }
            Command::TaskComplete { id } => {
                if self.void_task_id == Some(id) {
                    return Self::rejected("Void Task cannot be completed");
                }
                return match self.tasks.complete(pomotui_domain::TaskId::new(id)) {
                    Ok(()) => match self.record_task_version(id) {
                        Ok(()) => self.finish_task_mutation(mutation_key.as_deref()),
                        Err(error) => Self::rejected(error),
                    },
                    Err(error) => Self::task_rejected(error),
                };
            }
            Command::TaskReopen { id } => {
                return match self.tasks.reopen(pomotui_domain::TaskId::new(id)) {
                    Ok(()) => match self.record_task_version(id) {
                        Ok(()) => self.finish_task_mutation(mutation_key.as_deref()),
                        Err(error) => Self::rejected(error),
                    },
                    Err(error) => Self::task_rejected(error),
                };
            }
            Command::TaskDelete { id } => {
                if self.void_task_id == Some(id) {
                    return Self::rejected("Void Task cannot be deleted");
                }
                let id = pomotui_domain::TaskId::new(id);
                let result = self
                    .tasks
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
                    });
                return match result {
                    Ok(()) => match self.record_task_deletion(id.get()) {
                        Ok(()) => self.finish_task_mutation(mutation_key.as_deref()),
                        Err(error) => Self::rejected(error),
                    },
                    Err(error) => Self::rejected(error),
                };
            }
            Command::TaskSelect { id, stop_current } => {
                let id = pomotui_domain::TaskId::new(id);
                if let Err(error) = self.tasks.get(id) {
                    Err(error.to_string())
                } else if matches!(self.timer.current_session(), CurrentSession::Pending(_)) {
                    self.timer
                        .select_pending_task(id)
                        .map_err(|error| error.to_string())
                        .and_then(|()| self.apply_deferred_task_deletions())
                } else if stop_current {
                    let planned = self.timer.planned_seconds();
                    let transition = self.timer.stop(self.now);
                    self.apply_transition(transition, planned)
                        .and_then(|()| {
                            self.timer
                                .select_pending_task(id)
                                .map_err(|error| error.to_string())
                        })
                        .and_then(|()| self.apply_deferred_task_deletions())
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
                } else if self
                    .pending_review
                    .as_ref()
                    .is_some_and(|review| ids.contains(&review.session_id))
                {
                    Err("Session History with Pending Review cannot be deleted".into())
                } else if !ids
                    .iter()
                    .any(|id| self.history.records().iter().any(|record| record.id == *id))
                {
                    Err("Selected Session History entries no longer exist".into())
                } else {
                    for id in &ids {
                        if let Some(entity_id) = self.sync.session_entities.get(id).cloned() {
                            self.sync.records.push(SyncRecord::new(
                                RecordId::random(),
                                entity_id,
                                self.next_sync_mutation_time(),
                                RecordPayload::SessionDeleted,
                            ));
                        }
                    }
                    self.sync.records.sort();
                    self.history.delete(&ids);
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
            Command::ReviewSuccess { reflection } => {
                self.review_success(None, false, None, reflection)
            }
            Command::ReviewSuccessAssign {
                task_id,
                use_void,
                chain_entry_title,
                reflection,
            } => {
                if task_id.is_some() == use_void {
                    Err("Choose exactly one regular Task or the Void Task".into())
                } else {
                    self.review_success(task_id, use_void, chain_entry_title, reflection)
                }
            }
            Command::ActionChainCurrent => {
                return Response::Data {
                    value: serde_json::json!({
                        "id": self.current_chain_id,
                        "length": self.current_chain_length,
                        "links": self.chain_links.iter().map(|link| serde_json::json!({
                            "id": link.id,
                            "session_id": link.session_id,
                            "task_id": link.task_id,
                            "task_title": link.task_title,
                            "actual_seconds": link.actual_seconds,
                            "reflection": link.reflection,
                            "chain_entry_title": link.chain_entry_title,
                        })).collect::<Vec<_>>(),
                    }),
                };
            }
            Command::ReviewFailure {
                reflection,
                task_id,
                use_void,
                chain_entry_title,
            } => self.review_failure(&reflection, task_id, use_void, chain_entry_title),
            Command::ActionChainArchive => {
                return Response::Data {
                    value: serde_json::json!({
                        "chains": self.ended_chains.iter().rev().map(|chain| serde_json::json!({
                            "id": chain.id,
                            "length": chain.links.len(),
                            "links": chain.links,
                            "chain_break": chain.chain_break,
                        })).collect::<Vec<_>>(),
                    }),
                };
            }
            Command::EndedChainDelete { id } => {
                let before = self.ended_chains.len();
                self.ended_chains.retain(|chain| chain.id != id);
                if self.ended_chains.len() == before {
                    Err(format!("Ended Chain {id} does not exist"))
                } else {
                    self.reward_unlocks.retain(|reward| reward.chain_id != id);
                    Ok(())
                }
            }
            Command::ChainEntryEdit {
                id,
                reflection,
                chain_entry_title,
            } => self.edit_chain_entry(id, reflection, chain_entry_title),
            Command::RewardCreate {
                name,
                threshold,
                budget,
            } => {
                let name = name.trim();
                if name.is_empty() || threshold == 0 {
                    Err("Reward Milestone requires a name and positive threshold".into())
                } else {
                    self.reward_milestones.push(RewardMilestoneState {
                        id: self.next_reward_milestone_id,
                        name: name.to_owned(),
                        threshold,
                        budget,
                    });
                    self.next_reward_milestone_id = self.next_reward_milestone_id.saturating_add(1);
                    self.unlock_eligible_rewards();
                    Ok(())
                }
            }
            Command::RewardUpdate {
                id,
                name,
                threshold,
                budget,
            } => {
                let name = name.trim();
                if name.is_empty() || threshold == 0 {
                    Err("Reward Milestone requires a name and positive threshold".into())
                } else if let Some(milestone) = self
                    .reward_milestones
                    .iter_mut()
                    .find(|milestone| milestone.id == id)
                {
                    name.clone_into(&mut milestone.name);
                    milestone.threshold = threshold;
                    milestone.budget = budget;
                    self.unlock_eligible_rewards();
                    Ok(())
                } else {
                    Err(format!("Reward Milestone {id} does not exist"))
                }
            }
            Command::RewardDelete { id } => {
                let before = self.reward_milestones.len();
                self.reward_milestones
                    .retain(|milestone| milestone.id != id);
                if before == self.reward_milestones.len() {
                    Err(format!("Reward Milestone {id} does not exist"))
                } else {
                    Ok(())
                }
            }
            Command::RewardClaim { unlock_id } => {
                let Some(unlock) = self
                    .reward_unlocks
                    .iter_mut()
                    .find(|unlock| unlock.id == unlock_id)
                else {
                    return Self::rejected(format!("Reward unlock {unlock_id} does not exist"));
                };
                if unlock.state == "unlocked" {
                    unlock.state = "claimed".into();
                    unlock.claimed_at = Some(self.wall);
                    Ok(())
                } else if unlock.state == "claimed" {
                    Ok(())
                } else {
                    Err("Unavailable reward cannot be claimed".into())
                }
            }
            Command::Rewards => {
                return Response::Data {
                    value: serde_json::json!({
                        "milestones": self.reward_milestones,
                        "unlocks": self.reward_unlocks,
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
    #[serde(default)]
    data_format_version: u16,
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
    #[serde(default)]
    chain_links: Vec<ChainLinkState>,
    #[serde(default = "default_next_identity")]
    next_chain_entry_id: u64,
    #[serde(default)]
    void_task_id: Option<u64>,
    #[serde(default)]
    ended_chains: Vec<EndedChainState>,
    #[serde(default)]
    reward_milestones: Vec<RewardMilestoneState>,
    #[serde(default)]
    reward_unlocks: Vec<RewardUnlockState>,
    #[serde(default = "default_next_identity")]
    next_reward_milestone_id: u64,
    #[serde(default = "default_next_identity")]
    next_reward_unlock_id: u64,
    #[serde(default)]
    sync: SyncState,
}

const fn default_chain_id() -> u64 {
    1
}

const fn default_next_identity() -> u64 {
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
        let clock = PlatformClock::default();
        let persisted = Self {
            data_format_version: 2,
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
            chain_links: service.chain_links.clone(),
            next_chain_entry_id: service.next_chain_entry_id,
            void_task_id: service.void_task_id,
            ended_chains: service.ended_chains.clone(),
            reward_milestones: service.reward_milestones.clone(),
            reward_unlocks: service.reward_unlocks.clone(),
            next_reward_milestone_id: service.next_reward_milestone_id,
            next_reward_unlock_id: service.next_reward_unlock_id,
            sync: service.sync.clone(),
        };
        serde_json::to_string(&persisted).map_err(|error| error.to_string())
    }

    #[allow(clippy::too_many_lines)]
    fn decode(payload: &str) -> Result<Service, String> {
        let persisted: Self = serde_json::from_str(payload)
            .map_err(|error| format!("invalid durable state: {error}"))?;
        if persisted.data_format_version != 2 {
            return Err(
                "database format is incompatible with experimental synchronization; reset local pre-release data with `pomotui reset --all-data --confirm`"
                    .into(),
            );
        }
        let durations = SessionDurations::new(
            persisted.timer.focus_seconds,
            persisted.timer.short_break_seconds,
            persisted.timer.long_break_seconds,
        )
        .map_err(|error| error.to_string())?;
        let clock = PlatformClock::default();
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
            chain_links: persisted.chain_links,
            next_chain_entry_id: persisted.next_chain_entry_id,
            void_task_id: persisted.void_task_id,
            ended_chains: persisted.ended_chains,
            reward_milestones: persisted.reward_milestones,
            reward_unlocks: persisted.reward_unlocks,
            next_reward_milestone_id: persisted.next_reward_milestone_id,
            next_reward_unlock_id: persisted.next_reward_unlock_id,
            sync: persisted.sync,
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

const fn sync_kind(kind: DomainKind) -> SyncSessionKind {
    match kind {
        DomainKind::Focus => SyncSessionKind::Focus,
        DomainKind::ShortBreak => SyncSessionKind::ShortBreak,
        DomainKind::LongBreak => SyncSessionKind::LongBreak,
    }
}

const fn domain_sync_kind(kind: SyncSessionKind) -> DomainKind {
    match kind {
        SyncSessionKind::Focus => DomainKind::Focus,
        SyncSessionKind::ShortBreak => DomainKind::ShortBreak,
        SyncSessionKind::LongBreak => DomainKind::LongBreak,
    }
}

const fn sync_outcome(outcome: SessionOutcome) -> SyncSessionOutcome {
    match outcome {
        SessionOutcome::Completed => SyncSessionOutcome::Completed,
        SessionOutcome::Stopped => SyncSessionOutcome::Stopped,
        SessionOutcome::Skipped => SyncSessionOutcome::Skipped,
    }
}

const fn domain_sync_outcome(outcome: SyncSessionOutcome) -> SessionOutcome {
    match outcome {
        SyncSessionOutcome::Completed => SessionOutcome::Completed,
        SyncSessionOutcome::Stopped => SessionOutcome::Stopped,
        SyncSessionOutcome::Skipped => SessionOutcome::Skipped,
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
    use pomotui_cli::{parse as parse_cli, render as render_cli};
    use pomotui_platform::LinuxClock;
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

    fn create_empty_sync_file(path: &Path) {
        let document = SyncDocument::new(&[])
            .and_then(|document| document.to_json())
            .expect("empty sync document");
        std::fs::write(path, document).expect("create sync document");
    }

    #[test]
    fn one_task_converges_between_two_fresh_services_through_one_file() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");

        let (enable_first, _, _) = parse_cli(&[
            "sync".into(),
            "enable".into(),
            sync_path.display().to_string(),
        ])
        .expect("CLI enable command");
        first.handle(request(Some("enable-first"), enable_first));
        let (create_task, _, _) =
            parse_cli(&["task".into(), "create".into(), "Cross-device task".into()])
                .expect("CLI create command");
        first.handle(request(Some("create-task"), create_task));
        let first_task_id = match first.handle(request(None, Command::TaskList)) {
            Response::Data { value } => value
                .as_array()
                .expect("tasks")
                .iter()
                .find(|task| task["title"] == "Cross-device task")
                .and_then(|task| task["id"].as_u64())
                .expect("local Task ID"),
            response => panic!("unexpected Task list response: {response:?}"),
        };

        let (enable_second, _, _) = parse_cli(&[
            "sync".into(),
            "enable".into(),
            sync_path.display().to_string(),
        ])
        .expect("CLI enable command");
        second.handle(request(Some("enable-second"), enable_second));
        let (sync_now, _, _) = parse_cli(&["sync".into(), "now".into()]).expect("CLI sync now");
        let sync_response = second.handle(request(Some("sync-second"), sync_now));
        assert!(
            render_cli(&sync_response, false, false)
                .expect("human sync status")
                .contains("experimental")
        );
        let rendered: serde_json::Value = serde_json::from_str(
            &render_cli(&sync_response, true, false).expect("JSON sync status"),
        )
        .expect("structured JSON status");
        assert_eq!(rendered["value"]["stability"], "experimental");
        assert_eq!(
            rendered["value"]["capabilities"],
            serde_json::json!(["task_lifecycle", "session_history"])
        );
        second.handle(request(Some("sync-second-again"), Command::SyncNow));

        let Response::Data { value: tasks } = second.handle(request(None, Command::TaskList))
        else {
            panic!("Task list response");
        };
        let imported = tasks
            .as_array()
            .expect("tasks")
            .iter()
            .filter(|task| task["title"] == "Cross-device task")
            .collect::<Vec<_>>();
        assert_eq!(imported.len(), 1);
        assert_eq!(
            first_task_id, 1,
            "sync must not replace the local Task identity"
        );

        let document = std::fs::read_to_string(&sync_path).expect("sync document");
        assert!(document.ends_with('\n'));
        assert_eq!(
            document,
            std::fs::read_to_string(&sync_path).expect("stable document")
        );
        let json: serde_json::Value = serde_json::from_str(&document).expect("valid JSON");
        assert_eq!(json["format"], "pomotui.sync");
        assert_eq!(json["version"], 3);
        assert_eq!(json["integrity"]["record_count"], 1);
        assert_eq!(
            json["integrity"]["records_sha256"]
                .as_str()
                .expect("checksum")
                .len(),
            64
        );
        assert_eq!(json["records"].as_array().expect("records").len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn disabling_preserves_records_and_rebuild_recovers_a_valid_local_union() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-sync-rebuild-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("path with spaces.sync");
        create_empty_sync_file(&sync_path);
        let mut service = Service::open(&root.join("service.sqlite3")).expect("service");
        service.handle(request(
            Some("enable"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        service.handle(request(
            Some("create"),
            Command::TaskCreate {
                title: "Retained".into(),
            },
        ));
        let local_records = service.sync.records.len();

        service.handle(request(Some("disable"), Command::SyncDisable));
        assert_eq!(service.sync.records.len(), local_records);
        assert!(
            !sync_status(&mut service)["enabled"]
                .as_bool()
                .unwrap_or(true)
        );

        std::fs::remove_file(&sync_path).expect("remove exchange file");
        let missing = service.handle(request(
            Some("enable-missing"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert!(matches!(missing, Response::Error { .. }));
        assert!(
            !sync_path.exists(),
            "ordinary sync must not create a missing file"
        );

        std::fs::write(&sync_path, "truncated").expect("damage exchange file");
        service.handle(request(
            Some("enable-again"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert_eq!(
            std::fs::read_to_string(&sync_path).expect("unchanged invalid file"),
            "truncated"
        );
        service.handle(request(Some("rebuild"), Command::SyncRebuild));
        let rebuilt = std::fs::read_to_string(&sync_path).expect("rebuilt file");
        let document = SyncDocument::from_json(&rebuilt).expect("valid rebuilt document");
        assert_eq!(document.records().len(), local_records);
        let status = sync_status(&mut service);
        assert!(
            status["warning"]
                .as_str()
                .unwrap_or_default()
                .contains("unseen remote")
        );
        assert!(status["last_error"].is_null());

        let _ = std::fs::remove_dir_all(root);
    }

    fn sync_status(service: &mut Service) -> serde_json::Value {
        let Response::Data { value } = service.handle(request(None, Command::SyncStatus)) else {
            panic!("sync status response");
        };
        value
    }

    #[test]
    fn task_rename_completion_and_reopen_converge_between_services() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-task-updates-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        first.handle(request(
            Some("enable-first"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        second.handle(request(
            Some("enable-second"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        first.handle(request(
            Some("create"),
            Command::TaskCreate {
                title: "Draft".into(),
            },
        ));
        second.handle(request(Some("import-create"), Command::SyncNow));
        second.handle(request(
            Some("start-history"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        second.handle(request(Some("stop-history"), Command::Stop));

        first.handle(request(
            Some("rename"),
            Command::TaskRename {
                id: 1,
                title: "Publish".into(),
            },
        ));
        first.handle(request(Some("complete"), Command::TaskComplete { id: 1 }));
        second.handle(request(Some("import-updates"), Command::SyncNow));

        let Response::Data { value: tasks } = second.handle(request(None, Command::TaskList))
        else {
            panic!("Task list response");
        };
        let task = tasks
            .as_array()
            .expect("tasks")
            .iter()
            .find(|task| task["id"] == 1)
            .expect("imported Task");
        assert_eq!(task["title"], "Publish");
        assert_eq!(task["status"], "completed");
        let Response::Data { value: history } = second.handle(request(None, Command::History))
        else {
            panic!("History response");
        };
        assert_eq!(history[0]["task_title"], "Draft");

        second.handle(request(Some("reopen"), Command::TaskReopen { id: 1 }));
        first.handle(request(Some("import-reopen"), Command::SyncNow));
        let Response::Data { value: tasks } = first.handle(request(None, Command::TaskList)) else {
            panic!("Task list response");
        };
        let task = tasks
            .as_array()
            .expect("tasks")
            .iter()
            .find(|task| task["id"] == 1)
            .expect("local Task");
        assert_eq!(task["status"], "open");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn imported_task_deletion_waits_for_the_current_session_then_converges() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-task-delete-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        for (service, key) in [(&mut first, "enable-first"), (&mut second, "enable-second")] {
            service.handle(request(
                Some(key),
                Command::SyncEnable {
                    path: sync_path.clone(),
                },
            ));
        }
        first.handle(request(
            Some("create"),
            Command::TaskCreate {
                title: "Protected remote Task".into(),
            },
        ));
        second.handle(request(Some("import-create"), Command::SyncNow));
        second.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));

        first.handle(request(Some("delete"), Command::TaskDelete { id: 1 }));
        second.handle(request(Some("import-delete"), Command::SyncNow));
        let Response::Snapshot { snapshot } = second.handle(request(None, Command::Status)) else {
            panic!("status response");
        };
        assert!(snapshot.tasks.iter().any(|task| task.id == 1));
        assert_eq!(snapshot.current_task_id, Some(1));

        second.handle(request(Some("stop"), Command::Stop));
        let Response::Snapshot { snapshot } = second.handle(request(None, Command::Status)) else {
            panic!("status response");
        };
        assert!(snapshot.tasks.iter().any(|task| task.id == 1));
        second.handle(request(
            Some("create-replacement"),
            Command::TaskCreate {
                title: "Replacement".into(),
            },
        ));
        let Response::Snapshot { snapshot } = second.handle(request(None, Command::Status)) else {
            panic!("status response");
        };
        assert_eq!(snapshot.current_task_id, Some(1));
        second.handle(request(
            Some("start-replacement"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(2),
            },
        ));
        let Response::Data { value: tasks } = second.handle(request(None, Command::TaskList))
        else {
            panic!("Task list response");
        };
        assert!(
            !tasks
                .as_array()
                .expect("tasks")
                .iter()
                .any(|task| task["id"] == 1)
        );
        second.handle(request(Some("repeat-delete"), Command::SyncNow));
        let Response::Data { value: tasks } = second.handle(request(None, Command::TaskList))
        else {
            panic!("Task list response");
        };
        assert!(
            !tasks
                .as_array()
                .expect("tasks")
                .iter()
                .any(|task| task["id"] == 1)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn offline_task_versions_converge_deterministically_and_keep_same_titles_distinct() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-task-conflict-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let first_file = root.join("first.sync");
        let second_file = root.join("second.sync");
        create_empty_sync_file(&first_file);
        create_empty_sync_file(&second_file);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        first.handle(request(
            Some("enable-first"),
            Command::SyncEnable {
                path: first_file.clone(),
            },
        ));
        second.handle(request(
            Some("enable-second"),
            Command::SyncEnable {
                path: second_file.clone(),
            },
        ));
        for (service, key) in [(&mut first, "create-first"), (&mut second, "create-second")] {
            service.handle(request(
                Some(key),
                Command::TaskCreate {
                    title: "Same title".into(),
                },
            ));
        }

        std::fs::copy(&first_file, &second_file).expect("transport first file");
        second.handle(request(Some("merge-creates-second"), Command::SyncNow));
        std::fs::copy(&second_file, &first_file).expect("transport merged file");
        first.handle(request(Some("merge-creates-first"), Command::SyncNow));
        for service in [&mut first, &mut second] {
            let Response::Data { value: tasks } = service.handle(request(None, Command::TaskList))
            else {
                panic!("Task list response");
            };
            assert_eq!(
                tasks
                    .as_array()
                    .expect("tasks")
                    .iter()
                    .filter(|task| task["title"] == "Same title")
                    .count(),
                2
            );
        }

        first.handle(request(
            Some("rename-first"),
            Command::TaskRename {
                id: 1,
                title: "First edit".into(),
            },
        ));
        second.handle(request(
            Some("rename-second"),
            Command::TaskRename {
                id: 2,
                title: "Second edit".into(),
            },
        ));
        std::fs::copy(&first_file, &second_file).expect("transport conflicting file");
        second.handle(request(Some("merge-conflict-second"), Command::SyncNow));
        std::fs::copy(&second_file, &first_file).expect("transport resolved file");
        first.handle(request(Some("merge-conflict-first"), Command::SyncNow));

        let title = |service: &mut Service, id: u64| {
            let Response::Data { value: tasks } = service.handle(request(None, Command::TaskList))
            else {
                panic!("Task list response");
            };
            tasks
                .as_array()
                .expect("tasks")
                .iter()
                .find(|task| task["id"] == id)
                .and_then(|task| task["title"].as_str())
                .expect("Task title")
                .to_owned()
        };
        assert_eq!(title(&mut first, 1), title(&mut second, 2));

        first.handle(request(
            Some("correct-title"),
            Command::TaskRename {
                id: 1,
                title: "Corrected later".into(),
            },
        ));
        std::fs::copy(&first_file, &second_file).expect("transport correction");
        second.handle(request(Some("merge-correction"), Command::SyncNow));
        assert_eq!(title(&mut second, 2), "Corrected later");

        first.handle(request(
            Some("delete-offline"),
            Command::TaskDelete { id: 1 },
        ));
        second.handle(request(
            Some("stale-edit"),
            Command::TaskRename {
                id: 2,
                title: "Must not resurrect".into(),
            },
        ));
        std::fs::copy(&first_file, &second_file).expect("transport tombstone");
        second.handle(request(Some("merge-tombstone-second"), Command::SyncNow));
        std::fs::copy(&second_file, &first_file).expect("transport tombstone union");
        first.handle(request(Some("merge-tombstone-first"), Command::SyncNow));
        for service in [&mut first, &mut second] {
            let Response::Data { value: tasks } = service.handle(request(None, Command::TaskList))
            else {
                panic!("Task list response");
            };
            assert!(!tasks.as_array().expect("tasks").iter().any(|task| {
                task["title"] == "Corrected later" || task["title"] == "Must not resurrect"
            }));
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pre_lifecycle_database_requires_an_explicit_full_reset() {
        let service = Service::new();
        let encoded = PersistedService::encode(&service).expect("encode current state");
        let mut legacy: serde_json::Value = serde_json::from_str(&encoded).expect("durable JSON");
        legacy
            .as_object_mut()
            .expect("durable object")
            .remove("data_format_version");

        let error = PersistedService::decode(&legacy.to_string())
            .err()
            .expect("legacy state must be rejected");

        assert!(error.contains("pomotui reset --all-data --confirm"));
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
            assert_eq!(tasks.as_array().expect("array").len(), 2);
            assert!(
                tasks
                    .as_array()
                    .expect("array")
                    .iter()
                    .any(|task| task["title"] == "Durable Task")
            );
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
    fn session_history_syncs_by_global_identity_without_touching_local_cycle() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-session-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        second.handle(request(
            Some("local-task"),
            Command::TaskCreate {
                title: "Local".into(),
            },
        ));
        for (service, key) in [(&mut first, "enable-first"), (&mut second, "enable-second")] {
            service.handle(request(
                Some(key),
                Command::SyncEnable {
                    path: sync_path.clone(),
                },
            ));
        }
        first.handle(request(
            Some("shared-task"),
            Command::TaskCreate {
                title: "Shared".into(),
            },
        ));
        second.handle(request(Some("import-task"), Command::SyncNow));
        let imported_task = second
            .snapshot()
            .tasks
            .iter()
            .find(|task| task.title == "Shared")
            .expect("imported Task")
            .id;
        assert_ne!(
            imported_task, 1,
            "local numeric identities intentionally differ"
        );

        let planned = first.timer.planned_seconds();
        let transition = first.timer.start(first.now, Some(TaskId::new(1)));
        first
            .apply_transition(transition, planned)
            .expect("start Focus Session");
        first.now = first.now.saturating_add(37);
        first.wall = first.wall.saturating_add(37);
        let transition = first.timer.stop(first.now);
        first
            .apply_transition(transition, planned)
            .expect("stop Focus Session");
        first.handle(request(Some("export-history"), Command::SyncNow));

        let local_before = second.snapshot();
        second.handle(request(Some("import-history"), Command::SyncNow));
        second.handle(request(Some("retry-history"), Command::SyncNow));
        let snapshot = second.snapshot();
        assert_eq!(
            snapshot.recent_history.len(),
            1,
            "retry must not double count"
        );
        assert_eq!(snapshot.recent_history[0].actual_seconds, 37);
        assert_eq!(
            snapshot.recent_history[0].task_title.as_deref(),
            Some("Shared")
        );
        assert_eq!(
            second.history.records()[0].task_id,
            Some(TaskId::new(imported_task))
        );
        assert_eq!(snapshot.today.focus_seconds, 37);
        assert_eq!(snapshot.completed_rounds, local_before.completed_rounds);
        assert_eq!(snapshot.state, local_before.state);
        assert_eq!(snapshot.kind, local_before.kind);

        let source_id = first.snapshot().recent_history[0].id;
        first.handle(request(
            Some("delete-history"),
            Command::HistoryDelete {
                ids: vec![source_id],
            },
        ));
        first.handle(request(Some("export-deletion"), Command::SyncNow));
        second.handle(request(Some("import-deletion"), Command::SyncNow));
        assert!(second.snapshot().recent_history.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn every_ended_session_shape_converges_while_pending_review_stays_local() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-all-session-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        for (service, key) in [(&mut first, "enable-first"), (&mut second, "enable-second")] {
            service.handle(request(
                Some(key),
                Command::SyncEnable {
                    path: sync_path.clone(),
                },
            ));
        }
        first.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Attributed".into(),
            },
        ));
        let ended_at = first.wall;
        let shapes = [
            (DomainKind::Focus, SessionOutcome::Completed, 60, 60, true),
            (DomainKind::Focus, SessionOutcome::Stopped, 60, 31, true),
            (DomainKind::Focus, SessionOutcome::Skipped, 60, 0, false),
            (
                DomainKind::ShortBreak,
                SessionOutcome::Completed,
                10,
                10,
                false,
            ),
            (DomainKind::LongBreak, SessionOutcome::Stopped, 20, 4, false),
        ];
        for (index, (kind, outcome, planned, actual, attributed)) in shapes.into_iter().enumerate()
        {
            let id = u64::try_from(index).expect("small index") + 1;
            first.history.push(SessionRecord {
                id,
                ended_at: ended_at + i64::try_from(index).expect("small index"),
                kind,
                outcome,
                planned_seconds: planned,
                actual_seconds: actual,
                task_id: attributed.then_some(TaskId::new(1)),
                task_title: attributed.then(|| "Attributed".into()),
            });
            first.record_ended_session(id);
        }
        first.next_event_id = 6;
        first.pending_review = Some(PendingReviewState {
            session_id: 1,
            actual_seconds: 60,
            task_id: Some(1),
            task_title: Some("Attributed".into()),
        });
        first.handle(request(Some("export"), Command::SyncNow));
        second.handle(request(Some("import"), Command::SyncNow));

        assert_eq!(second.history.records().len(), 5);
        assert_eq!(second.snapshot().today.focus_seconds, 91);
        assert_eq!(second.snapshot().today.completed_rounds, 1);
        assert_eq!(second.snapshot().completed_rounds, 0);
        assert!(second.pending_review.is_none());
        assert!(first.pending_review.is_some());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn enabling_sync_backfills_existing_history_and_its_later_deletion() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-existing-history-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        first.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Existing work".into(),
            },
        ));
        first.history.push(SessionRecord {
            id: 1,
            ended_at: first.wall,
            kind: DomainKind::Focus,
            outcome: SessionOutcome::Stopped,
            planned_seconds: 1_500,
            actual_seconds: 420,
            task_id: Some(TaskId::new(1)),
            task_title: Some("Existing work".into()),
        });
        first.next_event_id = 2;

        first.handle(request(
            Some("enable-first"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        second.handle(request(
            Some("enable-second"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert_eq!(second.snapshot().recent_history.len(), 1);
        assert_eq!(second.snapshot().recent_history[0].actual_seconds, 420);

        first.handle(request(
            Some("delete"),
            Command::HistoryDelete { ids: vec![1] },
        ));
        first.handle(request(Some("export-delete"), Command::SyncNow));
        second.handle(request(Some("import-delete"), Command::SyncNow));
        assert!(second.snapshot().recent_history.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn existing_history_keeps_the_global_identity_of_an_already_deleted_task() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-deleted-task-history-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let sync_path = root.join("pomotui.sync");
        create_empty_sync_file(&sync_path);
        let mut first = Service::open(&root.join("first.sqlite3")).expect("first service");
        let deleted_task = first.tasks.create("Deleted work").expect("legacy Task");
        first.history.push(SessionRecord {
            id: 1,
            ended_at: first.wall,
            kind: DomainKind::Focus,
            outcome: SessionOutcome::Stopped,
            planned_seconds: 1_500,
            actual_seconds: 300,
            task_id: Some(deleted_task),
            task_title: Some("Deleted work".into()),
        });
        first
            .tasks
            .delete(deleted_task, None)
            .expect("delete legacy Task");
        first.next_event_id = 2;
        first.handle(request(
            Some("enable-first"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));

        let mut second = Service::open(&root.join("second.sqlite3")).expect("second service");
        second.handle(request(
            Some("enable-second"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        second.handle(request(Some("retry"), Command::SyncNow));
        assert_eq!(second.history.records().len(), 1);
        assert_eq!(second.history.records()[0].task_id, Some(TaskId::new(1)));
        assert_eq!(
            second.history.records()[0].task_title.as_deref(),
            Some("Deleted work")
        );
        assert!(
            second
                .snapshot()
                .tasks
                .iter()
                .all(|task| task.title != "Deleted work")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn durable_service_has_one_protected_void_identity_across_restarts() {
        let path = database_path().with_extension("void.sqlite3");
        let _ = std::fs::remove_file(&path);
        {
            let mut service = Service::open(&path).expect("service");
            let void_id = service.void_task_id.expect("Void identity");
            assert_eq!(
                service
                    .snapshot()
                    .tasks
                    .iter()
                    .filter(|task| task.id == void_id && task.title == "Void")
                    .count(),
                1
            );
            service.handle(request(
                Some("ordinary-void"),
                Command::TaskCreate {
                    title: "Void".into(),
                },
            ));
            assert_eq!(
                service
                    .snapshot()
                    .tasks
                    .iter()
                    .filter(|task| task.title == "Void")
                    .count(),
                2
            );
        }
        let service = Service::open(&path).expect("restart");
        assert_eq!(
            service
                .tasks
                .all()
                .iter()
                .filter(|task| task.id().get() == u64::MAX)
                .count(),
            1
        );
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
    fn successful_review_appends_one_chain_link_and_clears_pending_review() {
        let mut service = Service::new();
        service.reminders_enabled = false;
        service
            .configure_durations(SessionDurations::new(1, 1, 1).expect("durations"))
            .expect("configure");
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Implement review".into(),
            },
        ));
        let start = service.now;
        service.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.apply_observation(start + 1, service.wall + 1);

        let reviewed = service.handle(request(
            Some("review-success"),
            Command::ReviewSuccess {
                reflection: Some("Kept the slice small".into()),
            },
        ));
        let Response::Snapshot { snapshot } = reviewed else {
            panic!("review snapshot");
        };
        assert_eq!(snapshot.action_chain.length, 1);
        assert!(snapshot.pending_review.is_none());

        let Response::Data { value } = service.handle(request(None, Command::ActionChainCurrent))
        else {
            panic!("chain details");
        };
        assert_eq!(value["links"][0]["task_title"], "Implement review");
        assert_eq!(value["links"][0]["actual_seconds"], 1);
        assert_eq!(value["links"][0]["reflection"], "Kept the slice small");

        let replay = service.handle(request(
            Some("review-success"),
            Command::ReviewSuccess {
                reflection: Some("Duplicate".into()),
            },
        ));
        let Response::Snapshot { snapshot } = replay else {
            panic!("idempotent replay");
        };
        assert_eq!(snapshot.action_chain.length, 1);
    }

    #[test]
    fn early_stop_enters_review_only_when_explicitly_requested() {
        let mut service = Service::new();
        service.handle(request(
            Some("start-reviewed"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: None,
            },
        ));
        service.handle(request(Some("stop-reviewed"), Command::StopReview));
        let review = service.snapshot().pending_review.expect("pending review");
        assert_eq!(review.actual_seconds, 0);

        service.pending_review = None;
        service.handle(request(
            Some("start-unreviewed"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: None,
            },
        ));
        service.handle(request(Some("stop-unreviewed"), Command::Stop));
        assert!(service.snapshot().pending_review.is_none());
        assert_eq!(
            service
                .history
                .records()
                .last()
                .expect("history")
                .actual_seconds,
            0
        );
    }

    #[test]
    fn unattributed_review_can_use_the_protected_void_task() {
        let mut service = Service::new();
        service.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: None,
            },
        ));
        service.handle(request(Some("stop"), Command::StopReview));

        let response = service.handle(request(
            Some("void-review"),
            Command::ReviewSuccessAssign {
                task_id: None,
                use_void: true,
                chain_entry_title: Some("Explore the review flow".into()),
                reflection: Some("Found the next step".into()),
            },
        ));
        let Response::Snapshot { snapshot } = response else {
            panic!("successful Void review");
        };
        let void = snapshot
            .tasks
            .iter()
            .find(|task| task.title == "Void")
            .expect("system Void Task");
        assert_eq!(snapshot.recent_chain_links[0].task_title, "Void");
        assert_eq!(
            snapshot.recent_chain_links[0].chain_entry_title.as_deref(),
            Some("Explore the review flow")
        );
        assert!(matches!(
            service.handle(request(
                Some("rename-void"),
                Command::TaskRename {
                    id: void.id,
                    title: "Hidden".into(),
                },
            )),
            Response::Error {
                error: ProtocolError::Rejected { .. }
            }
        ));
        assert!(matches!(
            service.handle(request(
                Some("delete-void"),
                Command::TaskDelete { id: void.id }
            )),
            Response::Error {
                error: ProtocolError::Rejected { .. }
            }
        ));
    }

    #[test]
    fn pending_review_identifies_an_existing_void_attribution() {
        let mut service = Service::new();
        service.handle(request(
            Some("first-start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: None,
            },
        ));
        service.handle(request(Some("first-stop"), Command::StopReview));
        service.handle(request(
            Some("first-review"),
            Command::ReviewSuccessAssign {
                task_id: None,
                use_void: true,
                chain_entry_title: Some("Create Void identity".into()),
                reflection: None,
            },
        ));
        let void_id = service.void_task_id.expect("Void identity");

        service.handle(request(
            Some("second-start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(void_id),
            },
        ));
        service.handle(request(Some("second-stop"), Command::StopReview));

        let review = service.snapshot().pending_review.expect("Pending Review");
        assert_eq!(review.task_id, Some(void_id));
        assert!(review.is_void);
    }

    #[test]
    fn snapshot_exposes_the_complete_current_chain_in_chain_order() {
        let mut service = Service::new();
        service.chain_links = (1..=6)
            .map(|id| ChainLinkState {
                id,
                session_id: id,
                task_id: 1,
                task_title: format!("Step {id}"),
                actual_seconds: id * 60,
                reflection: None,
                chain_entry_title: None,
            })
            .collect();
        service.current_chain_length = 6;

        let links = service.snapshot().recent_chain_links;
        assert_eq!(links.len(), 6);
        assert_eq!(links[0].id, 1);
        assert_eq!(links[5].id, 6);
    }

    #[test]
    fn failed_review_archives_chain_break_and_starts_a_new_empty_chain() {
        let mut service = Service::new();
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Hard slice".into(),
            },
        ));
        service.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop"), Command::StopReview));

        assert!(matches!(
            service.handle(request(
                Some("missing-reflection"),
                Command::ReviewFailure {
                    reflection: " ".into(),
                    task_id: None,
                    use_void: false,
                    chain_entry_title: None,
                },
            )),
            Response::Error {
                error: ProtocolError::Rejected { .. }
            }
        ));
        let old_chain_id = service.snapshot().action_chain.id;
        let response = service.handle(request(
            Some("failure"),
            Command::ReviewFailure {
                reflection: "The slice was still too large".into(),
                task_id: None,
                use_void: false,
                chain_entry_title: None,
            },
        ));
        let Response::Snapshot { snapshot } = response else {
            panic!("failure snapshot");
        };
        assert_eq!(snapshot.action_chain.length, 0);
        assert_ne!(snapshot.action_chain.id, old_chain_id);
        assert!(snapshot.pending_review.is_none());
        assert_eq!(snapshot.recent_ended_chains[0].id, old_chain_id);
        assert_eq!(
            snapshot.recent_ended_chains[0].break_reflection,
            "The slice was still too large"
        );
    }

    #[test]
    fn chain_text_can_be_revised_without_changing_review_identity() {
        let mut service = Service::new();
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Stable identity".into(),
            },
        ));
        service.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop"), Command::StopReview));
        let pending_id = service.pending_review.as_ref().expect("pending").session_id;
        assert!(matches!(
            service.handle(request(
                Some("delete-pending"),
                Command::HistoryDelete {
                    ids: vec![pending_id],
                },
            )),
            Response::Error {
                error: ProtocolError::Rejected { .. }
            }
        ));
        service.handle(request(
            Some("review"),
            Command::ReviewSuccess {
                reflection: Some("First wording".into()),
            },
        ));
        let link_id = service.chain_links[0].id;
        let session_id = service.chain_links[0].session_id;
        service.handle(request(
            Some("edit"),
            Command::ChainEntryEdit {
                id: link_id,
                reflection: Some("Clearer wording".into()),
                chain_entry_title: None,
            },
        ));
        assert_eq!(
            service.chain_links[0].reflection.as_deref(),
            Some("Clearer wording")
        );
        assert_eq!(service.chain_links[0].session_id, session_id);
        assert_eq!(service.chain_links[0].task_title, "Stable identity");
    }

    #[test]
    fn task_backed_chain_entry_can_have_an_independent_display_title() {
        let mut service = Service::new();
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Stable task name".into(),
            },
        ));
        service.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop"), Command::StopReview));
        service.handle(request(
            Some("review"),
            Command::ReviewSuccess {
                reflection: Some("Keep this reflection".into()),
            },
        ));
        let link_id = service.chain_links[0].id;

        let response = service.handle(request(
            Some("edit-title"),
            Command::ChainEntryEdit {
                id: link_id,
                reflection: None,
                chain_entry_title: Some("A clearer chain step".into()),
            },
        ));

        let Response::Snapshot { snapshot } = response else {
            panic!("task-backed title edit should succeed");
        };
        assert_eq!(
            snapshot.recent_chain_links[0].chain_entry_title.as_deref(),
            Some("A clearer chain step")
        );
        assert_eq!(
            snapshot.recent_chain_links[0].reflection.as_deref(),
            Some("Keep this reflection")
        );
        assert_eq!(
            snapshot.recent_chain_links[0].task_title,
            "Stable task name"
        );
        assert_eq!(snapshot.tasks[0].title, "Stable task name");
    }

    #[test]
    fn reward_unlocks_once_at_threshold_and_can_be_claimed_once() {
        let mut service = Service::new();
        service.handle(request(
            Some("reward"),
            Command::RewardCreate {
                name: "Eat KFC".into(),
                threshold: 1,
                budget: Some(50),
            },
        ));
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Earn reward".into(),
            },
        ));
        service.handle(request(
            Some("start"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop"), Command::StopReview));
        let response = service.handle(request(
            Some("review"),
            Command::ReviewSuccess { reflection: None },
        ));
        let Response::Snapshot { snapshot } = response else {
            panic!("review snapshot");
        };
        assert_eq!(snapshot.current_chain_rewards.len(), 1);
        assert_eq!(snapshot.current_chain_rewards[0].name, "Eat KFC");
        assert_eq!(snapshot.current_chain_rewards[0].state, "unlocked");
        let unlock_id = snapshot.current_chain_rewards[0].id;

        service.handle(request(Some("claim"), Command::RewardClaim { unlock_id }));
        service.handle(request(
            Some("claim-retry"),
            Command::RewardClaim { unlock_id },
        ));
        assert_eq!(service.snapshot().current_chain_rewards[0].state, "claimed");
        assert_eq!(service.reward_unlocks.len(), 1);
    }

    #[test]
    fn snapshot_exposes_the_complete_reward_ladder_in_threshold_order() {
        let mut service = Service::new();
        service.handle(request(
            Some("reward-50"),
            Command::RewardCreate {
                name: "Day off".into(),
                threshold: 50,
                budget: None,
            },
        ));
        service.handle(request(
            Some("reward-10"),
            Command::RewardCreate {
                name: "KFC".into(),
                threshold: 10,
                budget: Some(80),
            },
        ));

        let milestones = service.snapshot().reward_milestones;
        assert_eq!(milestones.len(), 2);
        assert_eq!(milestones[0].threshold, 10);
        assert_eq!(milestones[0].name, "KFC");
        assert_eq!(milestones[0].budget, Some(80));
        assert_eq!(milestones[1].threshold, 50);
    }

    #[test]
    fn rewards_unlock_retroactively_and_settle_when_chain_fails() {
        let mut service = Service::new();
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Build chain".into(),
            },
        ));
        service.handle(request(
            Some("start-success"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-success"), Command::StopReview));
        service.handle(request(
            Some("success"),
            Command::ReviewSuccess { reflection: None },
        ));

        service.handle(request(
            Some("claimed-rule"),
            Command::RewardCreate {
                name: "Claimed reward".into(),
                threshold: 1,
                budget: None,
            },
        ));
        let claimed_id = service.snapshot().current_chain_rewards[0].id;
        service.handle(request(
            Some("claim"),
            Command::RewardClaim {
                unlock_id: claimed_id,
            },
        ));
        service.handle(request(
            Some("unclaimed-rule"),
            Command::RewardCreate {
                name: "At-risk reward".into(),
                threshold: 1,
                budget: None,
            },
        ));
        assert_eq!(service.snapshot().current_chain_rewards.len(), 2);

        service.handle(request(
            Some("start-failure"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-failure"), Command::StopReview));
        service.handle(request(
            Some("failure"),
            Command::ReviewFailure {
                reflection: "Stopped deliberately".into(),
                task_id: None,
                use_void: false,
                chain_entry_title: None,
            },
        ));

        let states = service
            .reward_unlocks
            .iter()
            .map(|unlock| unlock.state.as_str())
            .collect::<Vec<_>>();
        assert_eq!(states, ["claimed", "unavailable"]);
        assert!(service.snapshot().current_chain_rewards.is_empty());
    }

    #[test]
    fn snapshot_exposes_complete_recent_ended_chain_details_for_frontends() {
        let mut service = Service::new();
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Build archive details".into(),
            },
        ));
        service.handle(request(
            Some("start-success"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-success"), Command::StopReview));
        service.handle(request(
            Some("success"),
            Command::ReviewSuccess {
                reflection: Some("Completed the data projection".into()),
            },
        ));
        service.handle(request(
            Some("reward"),
            Command::RewardCreate {
                name: "Fancy coffee".into(),
                threshold: 1,
                budget: Some(35),
            },
        ));
        service.handle(request(
            Some("start-failure"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-failure"), Command::StopReview));
        service.handle(request(
            Some("failure"),
            Command::ReviewFailure {
                reflection: "The next slice was unclear".into(),
                task_id: None,
                use_void: false,
                chain_entry_title: None,
            },
        ));

        let snapshot = service.snapshot();
        let ended = &snapshot.recent_ended_chains[0];
        assert_eq!(ended.links.len(), 1);
        assert_eq!(ended.links[0].task_title, "Build archive details");
        assert_eq!(
            ended.links[0].reflection.as_deref(),
            Some("Completed the data projection")
        );
        assert_eq!(ended.break_chain_entry_title, None);
        assert_eq!(ended.rewards.len(), 1);
        assert_eq!(ended.rewards[0].name, "Fancy coffee");
        assert_eq!(ended.rewards[0].budget, Some(35));
        assert_eq!(ended.rewards[0].state, "unavailable");
    }

    #[test]
    fn deleting_an_ended_chain_removes_its_reward_history_but_preserves_session_history() {
        let mut service = Service::new();
        service.handle(request(
            Some("task"),
            Command::TaskCreate {
                title: "Retained work".into(),
            },
        ));
        service.handle(request(
            Some("start-success"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-success"), Command::StopReview));
        service.handle(request(
            Some("success"),
            Command::ReviewSuccess { reflection: None },
        ));
        service.handle(request(
            Some("reward"),
            Command::RewardCreate {
                name: "Coffee".into(),
                threshold: 1,
                budget: Some(35),
            },
        ));
        service.handle(request(
            Some("start-failure"),
            Command::Start {
                kind: SessionKind::Focus,
                task_id: Some(1),
            },
        ));
        service.handle(request(Some("stop-failure"), Command::StopReview));
        service.handle(request(
            Some("failure"),
            Command::ReviewFailure {
                reflection: "Stopped here".into(),
                task_id: None,
                use_void: false,
                chain_entry_title: None,
            },
        ));
        let before = service.snapshot();
        let ended_id = before.recent_ended_chains[0].id;
        assert_eq!(before.recent_history.len(), 2);
        assert_eq!(before.recent_ended_chains[0].rewards.len(), 1);

        let response = service.handle(request(
            Some("delete-ended"),
            Command::EndedChainDelete { id: ended_id },
        ));
        let Response::Snapshot { snapshot } = response else {
            panic!("delete should return the authoritative snapshot");
        };
        assert!(snapshot.recent_ended_chains.is_empty());
        assert_eq!(snapshot.recent_history.len(), 2);
        assert_eq!(
            snapshot.tasks[0].focus_seconds,
            before.tasks[0].focus_seconds
        );
        let Response::Snapshot { snapshot: replay } = service.handle(request(
            Some("delete-ended"),
            Command::EndedChainDelete { id: ended_id },
        )) else {
            panic!("idempotent retry should replay the authoritative snapshot");
        };
        assert!(replay.recent_ended_chains.is_empty());
        assert_eq!(replay.recent_history.len(), 2);

        assert!(matches!(
            service.handle(request(
                Some("delete-missing"),
                Command::EndedChainDelete { id: ended_id },
            )),
            Response::Error {
                error: ProtocolError::Rejected { message }
            } if message == format!("Ended Chain {ended_id} does not exist")
        ));
    }

    #[test]
    fn ended_chain_deletion_remains_deleted_after_sqlite_restart() {
        let path = database_path().with_extension("ended-delete.sqlite3");
        let _ = std::fs::remove_file(&path);
        {
            let mut service = Service::open(&path).expect("first service");
            service.handle(request(
                Some("task"),
                Command::TaskCreate {
                    title: "Durable archive".into(),
                },
            ));
            service.handle(request(
                Some("start"),
                Command::Start {
                    kind: SessionKind::Focus,
                    task_id: Some(1),
                },
            ));
            service.handle(request(Some("stop"), Command::StopReview));
            service.handle(request(
                Some("failure"),
                Command::ReviewFailure {
                    reflection: "Archive then delete".into(),
                    task_id: None,
                    use_void: false,
                    chain_entry_title: None,
                },
            ));
            let ended_id = service.snapshot().recent_ended_chains[0].id;
            assert!(matches!(
                service.handle(request(
                    Some("delete"),
                    Command::EndedChainDelete { id: ended_id },
                )),
                Response::Snapshot { .. }
            ));
        }
        {
            let service = Service::open(&path).expect("restarted service");
            let snapshot = service.snapshot();
            assert!(snapshot.recent_ended_chains.is_empty());
            assert_eq!(snapshot.recent_history.len(), 1);
            assert_eq!(
                snapshot.recent_history[0].task_title.as_deref(),
                Some("Durable archive")
            );
        }
        std::fs::remove_file(path).expect("cleanup");
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

    #[test]
    fn submitted_session_review_and_projected_chain_survive_sqlite_restart() {
        let path = database_path().with_extension("session-review-restart.sqlite3");
        let _ = std::fs::remove_file(&path);
        {
            let mut service = Service::open(&path).expect("service");
            service.handle(request(
                Some("create-review-task"),
                Command::TaskCreate {
                    title: "Restart Review".into(),
                },
            ));
            service.handle(request(
                Some("start-reviewed-session"),
                Command::Start {
                    kind: SessionKind::Focus,
                    task_id: Some(1),
                },
            ));
            service.handle(request(Some("stop-for-review"), Command::StopReview));
            assert!(matches!(
                service.handle(request(
                    Some("submit-session-review"),
                    Command::ReviewSuccess {
                        reflection: Some("Restarted cleanly".into()),
                    },
                )),
                Response::Snapshot { .. }
            ));
            assert_eq!(service.chain_links.len(), 1);
            assert_eq!(service.sync.session_review_entries.len(), 1);
        }

        let restarted = Service::open(&path).expect("restarted service");
        assert_eq!(restarted.chain_links.len(), 1);
        assert_eq!(restarted.sync.session_review_entries.len(), 1);
        assert_eq!(restarted.current_chain_length, 1);
        assert!(
            restarted
                .sync
                .records
                .iter()
                .any(|record| matches!(record.payload, RecordPayload::SessionReviewed { .. }))
        );
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn legacy_review_entry_mapping_name_survives_state_upgrade() {
        let mut service = Service::new();
        let review_identity = EntityId::parse("00000000-0000-0000-0000-000000000001")
            .expect("Session Review identity");
        service
            .sync
            .session_review_entries
            .insert(review_identity.clone(), 42);
        let current = PersistedService::encode(&service).expect("current state");
        let mut legacy: serde_json::Value = serde_json::from_str(&current).expect("state JSON");
        let sync = legacy["sync"].as_object_mut().expect("sync state");
        let mappings = sync
            .remove("session_review_entries")
            .expect("current mapping name");
        sync.insert("review_entries".into(), mappings);

        let upgraded = PersistedService::decode(&legacy.to_string()).expect("legacy state upgrade");

        assert_eq!(
            upgraded.sync.session_review_entries.get(&review_identity),
            Some(&42)
        );
    }

    #[test]
    fn session_review_records_and_projected_chains_roll_back_in_sqlite_together() {
        let identity = |value: u128| {
            EntityId::parse(&format!("00000000-0000-0000-0000-{value:012x}"))
                .expect("entity identity")
        };
        let record_id = |value: u128| {
            RecordId::parse(&format!("00000000-0000-0000-0000-{value:012x}"))
                .expect("record identity")
        };
        let task = identity(10);
        let session = identity(20);
        let incoming = vec![
            SyncRecord::new(
                record_id(1),
                task.clone(),
                MutationInstant::from_millis(1_000).expect("instant"),
                RecordPayload::TaskVersion {
                    title: "Atomic Review".into(),
                    status: SyncTaskStatus::Open,
                },
            ),
            SyncRecord::new(
                record_id(2),
                session.clone(),
                MutationInstant::from_millis(2_000).expect("instant"),
                RecordPayload::SessionEnded {
                    ended_at: 1_700_000_000,
                    kind: SyncSessionKind::Focus,
                    outcome: SyncSessionOutcome::Stopped,
                    planned_seconds: 1_500,
                    actual_seconds: 300,
                    task_entity_id: Some(task.clone()),
                    task_title: Some("Atomic Review".into()),
                },
            ),
            SyncRecord::new(
                record_id(3),
                identity(30),
                MutationInstant::from_millis(3_000).expect("instant"),
                RecordPayload::SessionReviewed {
                    session_entity_id: session,
                    judgment: SessionReviewJudgment::Successful,
                    task_entity_id: task,
                    task_title: "Atomic Review".into(),
                    actual_seconds: 300,
                    reflection: None,
                    chain_entry_title: None,
                },
            ),
        ];
        let plan = plan_sync(&[], &incoming).expect("valid incoming Review");
        let database = database_path().with_extension("atomic-session-review.sqlite3");
        let _ = std::fs::remove_file(&database);
        let mut service = Service::open(&database).expect("service");
        let sync_path = std::path::PathBuf::from("atomic-session-review.sync");
        service.sync.path = Some(sync_path.clone());
        service.persist(None).expect("persist synchronization path");
        rusqlite::Connection::open(&database)
            .expect("failure injection connection")
            .execute_batch(
                "CREATE TRIGGER reject_session_review_import
                 BEFORE UPDATE ON current_session
                 BEGIN
                     SELECT RAISE(ABORT, 'injected SQLite commit failure');
                 END;",
            )
            .expect("failure injection trigger");

        assert!(
            service
                .apply_sync_plan(&sync_path, &plan, incoming.len())
                .is_err()
        );
        assert!(service.sync.records.is_empty());
        assert!(service.history.records().is_empty());
        assert!(service.chain_links.is_empty());
        assert!(service.ended_chains.is_empty());
        assert_eq!(service.current_chain_length, 0);
        drop(service);

        let restarted = Service::open(&database).expect("restarted service");
        assert!(restarted.sync.records.is_empty());
        assert!(restarted.history.records().is_empty());
        assert!(restarted.chain_links.is_empty());
        assert!(restarted.ended_chains.is_empty());
        assert_eq!(restarted.current_chain_length, 0);
        std::fs::remove_file(database).expect("cleanup");
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
        assert_eq!(
            restarted
                .snapshot()
                .tasks
                .iter()
                .filter(|task| task.title != "Void")
                .count(),
            0
        );
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
