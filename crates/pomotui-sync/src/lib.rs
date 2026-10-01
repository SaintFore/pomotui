//! Provider-neutral synchronization document and convergence engine.

#![allow(clippy::missing_errors_doc)]

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub const FORMAT_VERSION: u16 = 3;
const FORMAT_NAME: &str = "pomotui.sync";

macro_rules! identity {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, String> {
                let parsed = uuid::Uuid::parse_str(value)
                    .map_err(|_| format!("invalid {} {value}", $label))?;
                if parsed.is_nil() {
                    return Err(format!("invalid {} {value}", $label));
                }
                Ok(Self(parsed.to_string()))
            }

            #[must_use]
            pub fn random() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

identity!(RecordId, "synchronization record identity");
identity!(EntityId, "synchronized entity identity");

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct MutationInstant(i64);

impl MutationInstant {
    pub fn from_millis(value: i64) -> Result<Self, String> {
        chrono::DateTime::from_timestamp_millis(value)
            .ok_or_else(|| format!("invalid synchronization mutation instant {value}"))?;
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for MutationInstant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_millis(i64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Completed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    Completed,
    Stopped,
    Skipped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewJudgment {
    Successful,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RecordPayload {
    TaskVersion {
        title: String,
        status: TaskStatus,
    },
    TaskDeleted,
    SessionEnded {
        ended_at: i64,
        kind: SessionKind,
        outcome: SessionOutcome,
        planned_seconds: u64,
        actual_seconds: u64,
        task_entity_id: Option<EntityId>,
        task_title: Option<String>,
    },
    SessionDeleted,
    SessionReviewed {
        session_entity_id: EntityId,
        judgment: ReviewJudgment,
        task_entity_id: EntityId,
        task_title: String,
        actual_seconds: u64,
        reflection: Option<String>,
        chain_entry_title: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Record {
    pub id: RecordId,
    pub entity_id: EntityId,
    pub mutation_time: MutationInstant,
    pub payload: RecordPayload,
}

impl Record {
    #[must_use]
    pub const fn new(
        id: RecordId,
        entity_id: EntityId,
        mutation_time: MutationInstant,
        payload: RecordPayload,
    ) -> Self {
        Self {
            id,
            entity_id,
            mutation_time,
            payload,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Integrity {
    record_count: usize,
    records_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Document {
    format: String,
    version: u16,
    integrity: Integrity,
    records: Vec<Record>,
}

impl Document {
    pub fn new(records: &[Record]) -> Result<Self, String> {
        let records = union(&[], records)?;
        validate_records(&records)?;
        Ok(Self {
            format: FORMAT_NAME.into(),
            version: FORMAT_VERSION,
            integrity: Integrity {
                record_count: records.len(),
                records_sha256: checksum(&records)?,
            },
            records,
        })
    }

    pub fn from_json(source: &str) -> Result<Self, String> {
        let document: Self = serde_json::from_str(source)
            .map_err(|error| format!("invalid sync document: {error}"))?;
        if document.format != FORMAT_NAME {
            return Err("unsupported sync document format".into());
        }
        if document.version != FORMAT_VERSION {
            return Err(format!(
                "unsupported sync document version {}; reset local pre-release data with `pomotui reset --all-data --confirm`",
                document.version
            ));
        }
        validate_records(&document.records)?;
        if document.integrity.record_count != document.records.len() {
            return Err("sync document integrity check failed".into());
        }
        if document.integrity.records_sha256 != checksum(&document.records)? {
            return Err("sync document checksum does not match its records".into());
        }
        Ok(document)
    }

    pub fn to_json(&self) -> Result<String, String> {
        let canonical = Self::new(&self.records)?;
        let mut source =
            serde_json::to_string_pretty(&canonical).map_err(|error| error.to_string())?;
        source.push('\n');
        Ok(source)
    }

    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    #[must_use]
    pub fn into_records(self) -> Vec<Record> {
        self.records
    }
}

pub fn union(left: &[Record], right: &[Record]) -> Result<Vec<Record>, String> {
    let mut records = BTreeMap::<RecordId, Record>::new();
    for record in left.iter().chain(right) {
        if let Some(existing) = records.get(&record.id)
            && existing != record
        {
            return Err(format!(
                "conflicting synchronization record {}",
                record.id.as_str()
            ));
        }
        records.insert(record.id.clone(), record.clone());
    }
    Ok(records.into_values().collect())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskProjection {
    Version {
        entity_id: EntityId,
        record_id: RecordId,
        title: String,
        status: TaskStatus,
    },
    Deleted {
        entity_id: EntityId,
        last_title: Option<String>,
    },
}

/// A validated synchronization result ready for the Timer Service to apply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncPlan {
    base_records: Vec<Record>,
    retained_records: Vec<Record>,
    task_projections: Vec<TaskProjection>,
    activity_projections: Vec<ActivityProjection>,
    review_projection: ReviewProjection,
}

impl SyncPlan {
    #[must_use]
    pub fn base_records(&self) -> &[Record] {
        &self.base_records
    }

    #[must_use]
    pub fn retained_records(&self) -> &[Record] {
        &self.retained_records
    }

    #[must_use]
    pub fn task_projections(&self) -> &[TaskProjection] {
        &self.task_projections
    }

    #[must_use]
    pub fn activity_projections(&self) -> &[ActivityProjection] {
        &self.activity_projections
    }

    #[must_use]
    pub const fn review_projection(&self) -> &ReviewProjection {
        &self.review_projection
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedReview {
    pub review_entity_id: EntityId,
    pub record_id: RecordId,
    pub session_entity_id: EntityId,
    pub judgment: ReviewJudgment,
    pub task_entity_id: EntityId,
    pub task_title: String,
    pub actual_seconds: u64,
    pub reflection: Option<String>,
    pub chain_entry_title: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectedChain {
    pub links: Vec<ProjectedReview>,
    pub chain_break: Option<ProjectedReview>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReviewProjection {
    pub ended_chains: Vec<ProjectedChain>,
    pub current_chain: ProjectedChain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityProjection {
    Session {
        entity_id: EntityId,
        ended_at: i64,
        kind: SessionKind,
        outcome: SessionOutcome,
        planned_seconds: u64,
        actual_seconds: u64,
        task_entity_id: Option<EntityId>,
        task_title: Option<String>,
    },
    Deleted {
        entity_id: EntityId,
    },
}

/// Produces the retained record union and its deterministic projections.
pub fn plan_sync(local: &[Record], incoming: &[Record]) -> Result<SyncPlan, String> {
    let retained_records = union(local, incoming)?;
    validate_records(&retained_records)?;
    let task_projections = project_tasks(&retained_records);
    let activity_projections = project_activity(&retained_records);
    let review_projection = project_reviews(&retained_records);
    Ok(SyncPlan {
        base_records: local.to_vec(),
        retained_records,
        task_projections,
        activity_projections,
        review_projection,
    })
}

/// Projects immutable Reviews in source-Session end order. Review identity is
/// the stable tie-breaker, so arrival order can never affect chain boundaries.
#[must_use]
pub fn project_reviews(records: &[Record]) -> ReviewProjection {
    let session_ends = records
        .iter()
        .filter_map(|record| match record.payload {
            RecordPayload::SessionEnded { ended_at, .. } => Some((&record.entity_id, ended_at)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut reviews = records
        .iter()
        .filter_map(|record| match &record.payload {
            RecordPayload::SessionReviewed {
                session_entity_id,
                judgment,
                task_entity_id,
                task_title,
                actual_seconds,
                reflection,
                chain_entry_title,
            } => Some((
                *session_ends.get(session_entity_id)?,
                record.id.clone(),
                ProjectedReview {
                    review_entity_id: record.entity_id.clone(),
                    record_id: record.id.clone(),
                    session_entity_id: session_entity_id.clone(),
                    judgment: *judgment,
                    task_entity_id: task_entity_id.clone(),
                    task_title: task_title.clone(),
                    actual_seconds: *actual_seconds,
                    reflection: reflection.clone(),
                    chain_entry_title: chain_entry_title.clone(),
                },
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    reviews.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    let mut projection = ReviewProjection::default();
    for (_, _, review) in reviews {
        if review.judgment == ReviewJudgment::Successful {
            projection.current_chain.links.push(review);
        } else {
            projection.current_chain.chain_break = Some(review);
            projection
                .ended_chains
                .push(std::mem::take(&mut projection.current_chain));
        }
    }
    projection
}

#[must_use]
pub fn project_tasks(records: &[Record]) -> Vec<TaskProjection> {
    let mut by_entity = BTreeMap::<EntityId, Vec<&Record>>::new();
    for record in records {
        by_entity
            .entry(record.entity_id.clone())
            .or_default()
            .push(record);
    }
    by_entity
        .into_iter()
        .filter_map(|(entity_id, records)| {
            if records
                .iter()
                .any(|record| matches!(record.payload, RecordPayload::TaskDeleted))
            {
                let last_title = records
                    .iter()
                    .filter_map(|record| match &record.payload {
                        RecordPayload::TaskVersion { title, .. } => {
                            Some((record.mutation_time, &record.id, title))
                        }
                        _ => None,
                    })
                    .max_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
                    .map(|(_, _, title)| title.clone());
                return Some(TaskProjection::Deleted {
                    entity_id,
                    last_title,
                });
            }
            records
                .into_iter()
                .filter_map(|record| match &record.payload {
                    RecordPayload::TaskVersion { title, status } => {
                        Some((record.mutation_time, &record.id, title, *status))
                    }
                    RecordPayload::TaskDeleted
                    | RecordPayload::SessionEnded { .. }
                    | RecordPayload::SessionDeleted
                    | RecordPayload::SessionReviewed { .. } => None,
                })
                .max_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
                .map(|(_, record_id, title, status)| TaskProjection::Version {
                    entity_id: entity_id.clone(),
                    record_id: record_id.clone(),
                    title: title.clone(),
                    status,
                })
        })
        .collect()
}

#[must_use]
pub fn project_activity(records: &[Record]) -> Vec<ActivityProjection> {
    let mut by_entity = BTreeMap::<EntityId, Vec<&Record>>::new();
    for record in records {
        if matches!(
            record.payload,
            RecordPayload::SessionEnded { .. } | RecordPayload::SessionDeleted
        ) {
            by_entity
                .entry(record.entity_id.clone())
                .or_default()
                .push(record);
        }
    }
    by_entity
        .into_iter()
        .filter_map(|(entity_id, records)| {
            if records
                .iter()
                .any(|record| matches!(record.payload, RecordPayload::SessionDeleted))
            {
                return Some(ActivityProjection::Deleted { entity_id });
            }
            records
                .into_iter()
                .find_map(|record| match &record.payload {
                    RecordPayload::SessionEnded {
                        ended_at,
                        kind,
                        outcome,
                        planned_seconds,
                        actual_seconds,
                        task_entity_id,
                        task_title,
                    } => Some(ActivityProjection::Session {
                        entity_id: entity_id.clone(),
                        ended_at: *ended_at,
                        kind: *kind,
                        outcome: *outcome,
                        planned_seconds: *planned_seconds,
                        actual_seconds: *actual_seconds,
                        task_entity_id: task_entity_id.clone(),
                        task_title: task_title.clone(),
                    }),
                    _ => None,
                })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn validate_records(records: &[Record]) -> Result<(), String> {
    let unique = union(&[], records)?;
    if unique.len() != records.len() {
        return Err("duplicate synchronization record identity".into());
    }
    for record in records {
        match &record.payload {
            RecordPayload::TaskVersion { title, .. } => {
                pomotui_domain::TaskTitle::parse(title)
                    .map_err(|error| format!("invalid synchronized Task: {error}"))?;
            }
            RecordPayload::SessionEnded {
                ended_at,
                kind,
                outcome,
                planned_seconds,
                actual_seconds,
                task_entity_id,
                task_title,
            } => {
                chrono::DateTime::from_timestamp(*ended_at, 0)
                    .ok_or_else(|| format!("invalid Session end time {ended_at}"))?;
                if task_entity_id.is_some() != task_title.is_some() {
                    return Err(
                        "synchronized Session Task identity and title must appear together".into(),
                    );
                }
                if !matches!(kind, SessionKind::Focus) && task_entity_id.is_some() {
                    return Err("synchronized Break Session cannot be attributed to a Task".into());
                }
                if matches!(outcome, SessionOutcome::Skipped) && *actual_seconds != 0 {
                    return Err(
                        "skipped synchronized Session must have zero actual duration".into(),
                    );
                }
                if matches!(outcome, SessionOutcome::Completed) && actual_seconds < planned_seconds
                {
                    return Err(
                        "completed synchronized Session cannot be shorter than planned".into(),
                    );
                }
            }
            RecordPayload::TaskDeleted | RecordPayload::SessionDeleted => {}
            RecordPayload::SessionReviewed {
                task_title,
                judgment,
                reflection,
                ..
            } => {
                pomotui_domain::TaskTitle::parse(task_title)
                    .map_err(|error| format!("invalid synchronized Review Task: {error}"))?;
                if *judgment == ReviewJudgment::Failed
                    && reflection
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                {
                    return Err("failed synchronized Review requires a Reflection".into());
                }
            }
        }
    }
    let mut entity_kinds = BTreeMap::<&EntityId, (&'static str, usize)>::new();
    for record in records {
        let (kind, immutable_fact) = match record.payload {
            RecordPayload::TaskVersion { .. } | RecordPayload::TaskDeleted => ("Task", false),
            RecordPayload::SessionEnded { .. } => ("Session", true),
            RecordPayload::SessionDeleted => ("Session", false),
            RecordPayload::SessionReviewed { .. } => ("Review", true),
        };
        let entry = entity_kinds.entry(&record.entity_id).or_insert((kind, 0));
        if entry.0 != kind {
            return Err("synchronization entity mixes Task and Session records".into());
        }
        if immutable_fact {
            entry.1 += 1;
            if entry.1 > 1 {
                return Err("synchronized Session has more than one ended fact".into());
            }
        }
    }
    let task_entities = records
        .iter()
        .filter_map(|record| {
            matches!(record.payload, RecordPayload::TaskVersion { .. }).then_some(&record.entity_id)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let sessions = records
        .iter()
        .filter_map(|record| match &record.payload {
            RecordPayload::SessionEnded { kind, outcome, .. } => {
                Some((&record.entity_id, (*kind, *outcome)))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut reviewed_sessions = std::collections::BTreeSet::new();
    for record in records {
        if let RecordPayload::SessionReviewed {
            session_entity_id,
            task_entity_id,
            actual_seconds,
            ..
        } = &record.payload
        {
            let Some((kind, outcome)) = sessions.get(session_entity_id) else {
                return Err("synchronized Review references unknown Session identity".into());
            };
            if *kind != SessionKind::Focus || *outcome == SessionOutcome::Skipped {
                return Err("only an ended reviewable Focus Session can be reviewed".into());
            }
            if !reviewed_sessions.insert(session_entity_id) {
                return Err("synchronized Session has more than one Review".into());
            }
            if !task_entities.contains(task_entity_id) {
                return Err("synchronized Review references unknown Task identity".into());
            }
            let session_actual = records.iter().find_map(|candidate| {
                (&candidate.entity_id == session_entity_id)
                    .then_some(match candidate.payload {
                        RecordPayload::SessionEnded { actual_seconds, .. } => Some(actual_seconds),
                        _ => None,
                    })
                    .flatten()
            });
            if session_actual != Some(*actual_seconds) {
                return Err("synchronized Review duration differs from its source Session".into());
            }
        }
    }
    for record in records {
        if let RecordPayload::SessionEnded {
            task_entity_id: Some(task_entity_id),
            ..
        } = &record.payload
            && !task_entities.contains(task_entity_id)
        {
            return Err(format!(
                "synchronized Session references unknown Task identity {}",
                task_entity_id.as_str()
            ));
        }
    }
    Ok(())
}

fn checksum(records: &[Record]) -> Result<String, String> {
    let canonical = serde_json::to_vec(records).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

impl fmt::Display for RecordId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
