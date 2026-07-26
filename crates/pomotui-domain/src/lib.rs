//! Pomotui's product model and infrastructure-independent ports.

use core::fmt;

mod tasks;

pub use tasks::{DailySummary, History, SessionRecord, Task, TaskError, TaskStatus, TaskStore};

/// Identifies the initial protocol-neutral domain model version.
pub const MODEL_VERSION: u16 = 1;

/// Stable identity of a Task.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TaskId(u64);

impl TaskId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The recommendation represented by a Session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionKind {
    Focus,
    ShortBreak,
    LongBreak,
}

/// Why a Session entered Session History.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionOutcome {
    Completed,
    Stopped,
    Skipped,
}

/// Default lengths used only when a new Session starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionDurations {
    focus: u64,
    short_break: u64,
    long_break: u64,
}

impl SessionDurations {
    /// Creates nonzero defaults for each Session kind.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidDuration`] if any duration is zero.
    pub fn new(focus: u64, short_break: u64, long_break: u64) -> Result<Self, DomainError> {
        if focus == 0 || short_break == 0 || long_break == 0 {
            return Err(DomainError::InvalidDuration);
        }
        Ok(Self {
            focus,
            short_break,
            long_break,
        })
    }

    const fn for_kind(self, kind: SessionKind) -> u64 {
        match kind {
            SessionKind::Focus => self.focus,
            SessionKind::ShortBreak => self.short_break,
            SessionKind::LongBreak => self.long_break,
        }
    }

    #[must_use]
    pub const fn focus(self) -> u64 {
        self.focus
    }

    #[must_use]
    pub const fn short_break(self) -> u64 {
        self.short_break
    }

    #[must_use]
    pub const fn long_break(self) -> u64 {
        self.long_break
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Pending {
        kind: SessionKind,
    },
    Running {
        kind: SessionKind,
        planned_seconds: u64,
        accumulated_seconds: u64,
        started_at: u64,
        task_id: Option<TaskId>,
    },
    Paused {
        kind: SessionKind,
        planned_seconds: u64,
        elapsed_seconds: u64,
        task_id: Option<TaskId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerState {
    pub session: SessionState,
    pub current_task: Option<TaskId>,
    pub completed_rounds: u8,
    pub rounds_per_cycle: u8,
    pub durations: SessionDurations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingSession {
    kind: SessionKind,
}

impl PendingSession {
    #[must_use]
    pub const fn kind(self) -> SessionKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunningSession {
    kind: SessionKind,
    planned_seconds: u64,
    accumulated_seconds: u64,
    started_at: u64,
    task_id: Option<TaskId>,
}

impl RunningSession {
    #[must_use]
    pub const fn kind(self) -> SessionKind {
        self.kind
    }

    #[must_use]
    pub const fn task_id(self) -> Option<TaskId> {
        self.task_id
    }

    #[must_use]
    pub const fn planned_seconds(self) -> u64 {
        self.planned_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PausedSession {
    kind: SessionKind,
    planned_seconds: u64,
    elapsed_seconds: u64,
    task_id: Option<TaskId>,
}

impl PausedSession {
    #[must_use]
    pub const fn kind(self) -> SessionKind {
        self.kind
    }

    #[must_use]
    pub const fn task_id(self) -> Option<TaskId> {
        self.task_id
    }

    #[must_use]
    pub const fn planned_seconds(self) -> u64 {
        self.planned_seconds
    }
}

/// The one shared Session visible to every Timer Frontend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentSession {
    Pending(PendingSession),
    Running(RunningSession),
    Paused(PausedSession),
}

/// Durable facts produced by a successful domain transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainEvent {
    SessionEnded {
        kind: SessionKind,
        outcome: SessionOutcome,
        actual_seconds: u64,
        task_id: Option<TaskId>,
    },
}

/// Observable result of advancing the state machine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Transition {
    events: Vec<DomainEvent>,
}

impl Transition {
    const fn none() -> Self {
        Self { events: Vec::new() }
    }

    fn ended(event: DomainEvent) -> Self {
        Self {
            events: vec![event],
        }
    }

    #[must_use]
    pub fn events(&self) -> &[DomainEvent] {
        &self.events
    }
}

/// Position within the configurable Focus Cycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusCycle {
    completed_rounds: u8,
    rounds_per_cycle: u8,
}

impl FocusCycle {
    #[must_use]
    pub const fn completed_rounds(self) -> u8 {
        self.completed_rounds
    }

    #[must_use]
    pub const fn rounds_per_cycle(self) -> u8 {
        self.rounds_per_cycle
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainError {
    InvalidDuration,
    InvalidCycleLength,
    InvalidTransition,
    TimeMovedBackwards,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidDuration => "Session durations must be nonzero",
            Self::InvalidCycleLength => "Focus Cycle length must be nonzero",
            Self::InvalidTransition => "operation is invalid for the Current Session",
            Self::TimeMovedBackwards => "elapsed-time observation moved backwards",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for DomainError {}

/// Pure state machine for the Current Session and Focus Cycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timer {
    current: CurrentSession,
    current_task: Option<TaskId>,
    cycle: FocusCycle,
    durations: SessionDurations,
}

impl Timer {
    /// Creates a Timer with a Pending Focus Session.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidCycleLength`] for a zero-length cycle.
    pub fn new(durations: SessionDurations, rounds_per_cycle: u8) -> Result<Self, DomainError> {
        if rounds_per_cycle == 0 {
            return Err(DomainError::InvalidCycleLength);
        }
        Ok(Self {
            current: CurrentSession::Pending(PendingSession {
                kind: SessionKind::Focus,
            }),
            current_task: None,
            cycle: FocusCycle {
                completed_rounds: 0,
                rounds_per_cycle,
            },
            durations,
        })
    }

    /// Restores an invariant-checked durable Timer state.
    ///
    /// # Errors
    ///
    /// Returns a validation error for invalid cycle values, zero durations, or
    /// impossible Session values.
    pub fn restore(state: TimerState) -> Result<Self, DomainError> {
        if state.rounds_per_cycle == 0 || state.completed_rounds > state.rounds_per_cycle {
            return Err(DomainError::InvalidCycleLength);
        }
        let current = match state.session {
            SessionState::Pending { kind } => CurrentSession::Pending(PendingSession { kind }),
            SessionState::Running {
                kind,
                planned_seconds,
                accumulated_seconds,
                started_at,
                task_id,
            } => {
                if planned_seconds == 0 {
                    return Err(DomainError::InvalidDuration);
                }
                CurrentSession::Running(RunningSession {
                    kind,
                    planned_seconds,
                    accumulated_seconds,
                    started_at,
                    task_id,
                })
            }
            SessionState::Paused {
                kind,
                planned_seconds,
                elapsed_seconds,
                task_id,
            } => {
                if planned_seconds == 0 || elapsed_seconds >= planned_seconds {
                    return Err(DomainError::InvalidDuration);
                }
                CurrentSession::Paused(PausedSession {
                    kind,
                    planned_seconds,
                    elapsed_seconds,
                    task_id,
                })
            }
        };
        Ok(Self {
            current,
            current_task: state.current_task,
            cycle: FocusCycle {
                completed_rounds: state.completed_rounds,
                rounds_per_cycle: state.rounds_per_cycle,
            },
            durations: state.durations,
        })
    }

    #[must_use]
    pub const fn state(&self) -> TimerState {
        let session = match self.current {
            CurrentSession::Pending(session) => SessionState::Pending { kind: session.kind },
            CurrentSession::Running(session) => SessionState::Running {
                kind: session.kind,
                planned_seconds: session.planned_seconds,
                accumulated_seconds: session.accumulated_seconds,
                started_at: session.started_at,
                task_id: session.task_id,
            },
            CurrentSession::Paused(session) => SessionState::Paused {
                kind: session.kind,
                planned_seconds: session.planned_seconds,
                elapsed_seconds: session.elapsed_seconds,
                task_id: session.task_id,
            },
        };
        TimerState {
            session,
            current_task: self.current_task,
            completed_rounds: self.cycle.completed_rounds,
            rounds_per_cycle: self.cycle.rounds_per_cycle,
            durations: self.durations,
        }
    }

    pub const fn set_durations(&mut self, durations: SessionDurations) {
        self.durations = durations;
    }

    /// Updates the configurable Focus Cycle length.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidCycleLength`] for zero.
    pub fn set_rounds_per_cycle(&mut self, rounds: u8) -> Result<(), DomainError> {
        if rounds == 0 {
            return Err(DomainError::InvalidCycleLength);
        }
        self.cycle.rounds_per_cycle = rounds;
        self.cycle.completed_rounds = self.cycle.completed_rounds.min(rounds);
        Ok(())
    }

    #[must_use]
    pub const fn current_session(&self) -> CurrentSession {
        self.current
    }

    #[must_use]
    pub const fn focus_cycle(&self) -> FocusCycle {
        self.cycle
    }

    #[must_use]
    pub const fn current_task(&self) -> Option<TaskId> {
        self.current_task
    }

    #[must_use]
    pub const fn planned_seconds(&self) -> u64 {
        match self.current {
            CurrentSession::Pending(session) => self.durations.for_kind(session.kind),
            CurrentSession::Running(session) => session.planned_seconds,
            CurrentSession::Paused(session) => session.planned_seconds,
        }
    }

    /// Starts the Pending Session at an elapsed-time observation.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidTransition`] unless the Current Session is
    /// Pending, or when a Task is supplied for a Break Session.
    pub fn start(&mut self, now: u64, task_id: Option<TaskId>) -> Result<Transition, DomainError> {
        let CurrentSession::Pending(pending) = self.current else {
            return Err(DomainError::InvalidTransition);
        };
        if pending.kind != SessionKind::Focus && task_id.is_some() {
            return Err(DomainError::InvalidTransition);
        }
        if pending.kind == SessionKind::Focus {
            self.current_task = task_id;
        }
        self.current = CurrentSession::Running(RunningSession {
            kind: pending.kind,
            planned_seconds: self.durations.for_kind(pending.kind),
            accumulated_seconds: 0,
            started_at: now,
            task_id: if pending.kind == SessionKind::Focus {
                self.current_task
            } else {
                None
            },
        });
        Ok(Transition::none())
    }

    /// Freezes a Running Session.
    ///
    /// # Errors
    ///
    /// Returns an error unless the Session is Running or when time moved
    /// backwards.
    pub fn pause(&mut self, now: u64) -> Result<Transition, DomainError> {
        let CurrentSession::Running(running) = self.current else {
            return Err(DomainError::InvalidTransition);
        };
        let elapsed = running.elapsed_at(now)?;
        if elapsed >= running.planned_seconds {
            return Ok(self.complete(running));
        }
        self.current = CurrentSession::Paused(PausedSession {
            kind: running.kind,
            planned_seconds: running.planned_seconds,
            elapsed_seconds: elapsed,
            task_id: running.task_id,
        });
        Ok(Transition::none())
    }

    /// Resumes a Paused Session.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidTransition`] unless the Session is Paused.
    pub fn resume(&mut self, now: u64) -> Result<Transition, DomainError> {
        let CurrentSession::Paused(paused) = self.current else {
            return Err(DomainError::InvalidTransition);
        };
        self.current = CurrentSession::Running(RunningSession {
            kind: paused.kind,
            planned_seconds: paused.planned_seconds,
            accumulated_seconds: paused.elapsed_seconds,
            started_at: now,
            task_id: paused.task_id,
        });
        Ok(Transition::none())
    }

    /// Stops a Running or Paused Session and preserves its recommendation.
    ///
    /// # Errors
    ///
    /// Returns an error for a Pending Session or when time moved backwards.
    pub fn stop(&mut self, now: u64) -> Result<Transition, DomainError> {
        let (kind, actual_seconds, task_id) = match self.current {
            CurrentSession::Running(running) => (
                running.kind,
                running.elapsed_at(now)?.min(running.planned_seconds),
                running.task_id,
            ),
            CurrentSession::Paused(paused) => (paused.kind, paused.elapsed_seconds, paused.task_id),
            CurrentSession::Pending(_) => return Err(DomainError::InvalidTransition),
        };
        self.current = CurrentSession::Pending(PendingSession { kind });
        Ok(Transition::ended(DomainEvent::SessionEnded {
            kind,
            outcome: SessionOutcome::Stopped,
            actual_seconds,
            task_id,
        }))
    }

    /// Skips a Pending Session without starting it.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidTransition`] unless the Session is Pending.
    pub fn skip(&mut self) -> Result<Transition, DomainError> {
        let CurrentSession::Pending(pending) = self.current else {
            return Err(DomainError::InvalidTransition);
        };
        let kind = pending.kind;
        let following = match kind {
            SessionKind::Focus => {
                if self.cycle.completed_rounds == self.cycle.rounds_per_cycle {
                    SessionKind::LongBreak
                } else {
                    SessionKind::ShortBreak
                }
            }
            SessionKind::ShortBreak | SessionKind::LongBreak => SessionKind::Focus,
        };
        self.current = CurrentSession::Pending(PendingSession { kind: following });
        Ok(Transition::ended(DomainEvent::SessionEnded {
            kind,
            outcome: SessionOutcome::Skipped,
            actual_seconds: 0,
            task_id: None,
        }))
    }

    /// Applies a deadline transition if the Running Session is due.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::TimeMovedBackwards`] for an invalid observation.
    pub fn advance(&mut self, now: u64) -> Result<Transition, DomainError> {
        let CurrentSession::Running(running) = self.current else {
            return Ok(Transition::none());
        };
        if running.elapsed_at(now)? < running.planned_seconds {
            return Ok(Transition::none());
        }
        Ok(self.complete(running))
    }

    #[must_use]
    pub fn remaining_seconds(&self, now: u64) -> u64 {
        match self.current {
            CurrentSession::Pending(pending) => self.durations.for_kind(pending.kind),
            CurrentSession::Paused(paused) => paused
                .planned_seconds
                .saturating_sub(paused.elapsed_seconds),
            CurrentSession::Running(running) => running.planned_seconds.saturating_sub(
                running
                    .elapsed_at(now)
                    .unwrap_or(running.accumulated_seconds),
            ),
        }
    }

    fn complete(&mut self, running: RunningSession) -> Transition {
        let next = match running.kind {
            SessionKind::Focus => {
                self.cycle.completed_rounds = self
                    .cycle
                    .completed_rounds
                    .saturating_add(1)
                    .min(self.cycle.rounds_per_cycle);
                if self.cycle.completed_rounds == self.cycle.rounds_per_cycle {
                    SessionKind::LongBreak
                } else {
                    SessionKind::ShortBreak
                }
            }
            SessionKind::ShortBreak => SessionKind::Focus,
            SessionKind::LongBreak => {
                self.cycle.completed_rounds = 0;
                SessionKind::Focus
            }
        };
        self.current = CurrentSession::Pending(PendingSession { kind: next });
        Transition::ended(DomainEvent::SessionEnded {
            kind: running.kind,
            outcome: SessionOutcome::Completed,
            actual_seconds: running.planned_seconds,
            task_id: running.task_id,
        })
    }
}

impl RunningSession {
    fn elapsed_at(self, now: u64) -> Result<u64, DomainError> {
        let since_start = now
            .checked_sub(self.started_at)
            .ok_or(DomainError::TimeMovedBackwards)?;
        Ok(self.accumulated_seconds.saturating_add(since_start))
    }
}
