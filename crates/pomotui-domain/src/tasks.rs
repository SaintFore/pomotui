#![allow(clippy::missing_errors_doc)]

use crate::{DomainEvent, SessionKind, SessionOutcome, TaskId};
use core::fmt;
use unicode_normalization::UnicodeNormalization;
use unicode_width::UnicodeWidthStr;

const MAX_TASK_TITLE_BYTES: usize = 256;
const MAX_TASK_TITLE_COLUMNS: usize = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    Open,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    id: TaskId,
    title: TaskTitle,
    status: TaskStatus,
}

impl Task {
    #[must_use]
    pub const fn id(&self) -> TaskId {
        self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    #[must_use]
    pub const fn status(&self) -> TaskStatus {
        self.status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskTitle(String);

impl TaskTitle {
    pub fn parse(title: &str) -> Result<Self, TaskError> {
        let normalized: String = title.trim().nfc().collect();
        Self::validate(normalized)
    }

    fn restore(title: String) -> Result<Self, TaskError> {
        Self::validate(title)
    }

    fn validate(title: String) -> Result<Self, TaskError> {
        if title.trim().is_empty() {
            return Err(TaskError::EmptyTitle);
        }
        if title.chars().any(is_unsafe_title_character) {
            return Err(TaskError::UnsafeTitleCharacter);
        }
        let bytes = title.len();
        if bytes > MAX_TASK_TITLE_BYTES {
            return Err(TaskError::TitleTooLong {
                max: MAX_TASK_TITLE_BYTES,
                actual: bytes,
            });
        }
        let columns = UnicodeWidthStr::width(title.as_str());
        if columns > MAX_TASK_TITLE_COLUMNS {
            return Err(TaskError::TitleTooWide {
                max: MAX_TASK_TITLE_COLUMNS,
                actual: columns,
            });
        }
        Ok(Self(title))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_unsafe_title_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{feff}'
        )
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskStore {
    tasks: Vec<Task>,
    next_id: u64,
}

impl TaskStore {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            next_id: 1,
        }
    }

    pub fn restore(
        tasks: Vec<(TaskId, String, TaskStatus)>,
        next_id: u64,
    ) -> Result<Self, TaskError> {
        if next_id == 0 {
            return Err(TaskError::IdExhausted);
        }
        let mut restored = Vec::with_capacity(tasks.len());
        for (id, title, status) in tasks {
            restored.push(Task {
                id,
                title: TaskTitle::restore(title)?,
                status,
            });
        }
        Ok(Self {
            tasks: restored,
            next_id,
        })
    }

    #[must_use]
    pub const fn next_id(&self) -> u64 {
        self.next_id
    }

    pub fn create(&mut self, title: impl Into<String>) -> Result<TaskId, TaskError> {
        let title = TaskTitle::parse(&title.into())?;
        let id = TaskId::new(self.next_id);
        self.next_id = self.next_id.checked_add(1).ok_or(TaskError::IdExhausted)?;
        self.tasks.push(Task {
            id,
            title,
            status: TaskStatus::Open,
        });
        Ok(id)
    }

    #[must_use]
    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    pub fn get(&self, id: TaskId) -> Result<&Task, TaskError> {
        self.tasks
            .iter()
            .find(|task| task.id == id)
            .ok_or(TaskError::NotFound(id))
    }

    pub fn resolve_title(&self, title: &str) -> Result<TaskId, TaskError> {
        let title = TaskTitle::parse(title)?;
        let matches: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| task.title == title)
            .map(|task| task.id)
            .collect();
        match matches.as_slice() {
            [] => Err(TaskError::TitleNotFound(title.as_str().to_owned())),
            [id] => Ok(*id),
            _ => Err(TaskError::AmbiguousTitle(matches)),
        }
    }

    pub fn rename(&mut self, id: TaskId, title: impl Into<String>) -> Result<(), TaskError> {
        let title = TaskTitle::parse(&title.into())?;
        self.task_mut(id)?.title = title;
        Ok(())
    }

    pub fn complete(&mut self, id: TaskId) -> Result<(), TaskError> {
        self.task_mut(id)?.status = TaskStatus::Completed;
        Ok(())
    }

    pub fn reopen(&mut self, id: TaskId) -> Result<(), TaskError> {
        self.task_mut(id)?.status = TaskStatus::Open;
        Ok(())
    }

    pub fn delete(&mut self, id: TaskId, current_task: Option<TaskId>) -> Result<Task, TaskError> {
        if current_task == Some(id) {
            return Err(TaskError::ReferencedByCurrentSession(id));
        }
        let index = self
            .tasks
            .iter()
            .position(|task| task.id == id)
            .ok_or(TaskError::NotFound(id))?;
        Ok(self.tasks.remove(index))
    }

    fn task_mut(&mut self, id: TaskId) -> Result<&mut Task, TaskError> {
        self.tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or(TaskError::NotFound(id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskError {
    EmptyTitle,
    UnsafeTitleCharacter,
    TitleTooLong { max: usize, actual: usize },
    TitleTooWide { max: usize, actual: usize },
    IdExhausted,
    NotFound(TaskId),
    TitleNotFound(String),
    AmbiguousTitle(Vec<TaskId>),
    ReferencedByCurrentSession(TaskId),
}

impl fmt::Display for TaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TaskError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRecord {
    pub id: u64,
    pub ended_at: i64,
    pub kind: SessionKind,
    pub outcome: SessionOutcome,
    pub planned_seconds: u64,
    pub actual_seconds: u64,
    pub task_id: Option<TaskId>,
    pub task_title: Option<String>,
}

impl SessionRecord {
    #[must_use]
    pub fn from_event(
        event: DomainEvent,
        id: u64,
        ended_at: i64,
        planned_seconds: u64,
        tasks: &TaskStore,
    ) -> Self {
        let DomainEvent::SessionEnded {
            kind,
            outcome,
            actual_seconds,
            task_id,
        } = event;
        let task_title = task_id
            .and_then(|id| tasks.get(id).ok())
            .map(|task| task.title.as_str().to_owned());
        Self {
            id,
            ended_at,
            kind,
            outcome,
            planned_seconds,
            actual_seconds,
            task_id,
            task_title,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct History {
    records: Vec<SessionRecord>,
}

impl History {
    #[must_use]
    pub fn restore(records: Vec<SessionRecord>) -> Self {
        Self { records }
    }

    pub fn push(&mut self, record: SessionRecord) {
        self.records.push(record);
    }

    pub fn delete(&mut self, ids: &[u64]) -> usize {
        let before = self.records.len();
        self.records.retain(|record| !ids.contains(&record.id));
        before.saturating_sub(self.records.len())
    }

    /// Attributes a previously unattributed Session History record.
    pub fn attribute(
        &mut self,
        session_id: u64,
        task_id: TaskId,
        task_title: &str,
    ) -> Result<(), &'static str> {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.id == session_id)
        else {
            return Err("Session History entry does not exist");
        };
        if record.task_id.is_some() {
            return Err("Session History entry is already attributed");
        }
        record.task_id = Some(task_id);
        record.task_title = Some(task_title.to_owned());
        Ok(())
    }

    #[must_use]
    pub fn records(&self) -> &[SessionRecord] {
        &self.records
    }

    #[must_use]
    pub fn summarize(&self, day_starts: [i64; 8]) -> DailySummary {
        let mut focus_seconds = [0_u64; 7];
        let mut completed_rounds = [0_u32; 7];
        for record in &self.records {
            for index in 0..7 {
                if record.ended_at >= day_starts[index] && record.ended_at < day_starts[index + 1] {
                    if record.kind == SessionKind::Focus {
                        focus_seconds[index] =
                            focus_seconds[index].saturating_add(record.actual_seconds);
                        if record.outcome == SessionOutcome::Completed {
                            completed_rounds[index] = completed_rounds[index].saturating_add(1);
                        }
                    }
                    break;
                }
            }
        }
        DailySummary {
            focus_seconds,
            completed_rounds,
        }
    }

    #[must_use]
    pub fn focus_seconds_for_task(&self, id: TaskId) -> u64 {
        self.records
            .iter()
            .filter(|record| record.kind == SessionKind::Focus && record.task_id == Some(id))
            .map(|record| record.actual_seconds)
            .sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DailySummary {
    pub focus_seconds: [u64; 7],
    pub completed_rounds: [u32; 7],
}

impl DailySummary {
    #[must_use]
    pub fn average_focus_seconds(self) -> u64 {
        self.focus_seconds.iter().sum::<u64>() / 7
    }
}
