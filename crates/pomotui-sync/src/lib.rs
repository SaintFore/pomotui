//! Provider-neutral synchronization document and convergence engine.

#![allow(clippy::missing_errors_doc)]

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub const FORMAT_VERSION: u16 = 2;
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

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RecordPayload {
    TaskVersion { title: String, status: TaskStatus },
    TaskDeleted,
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
    },
}

/// A validated synchronization result ready for the Timer Service to apply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncPlan {
    base_records: Vec<Record>,
    retained_records: Vec<Record>,
    task_projections: Vec<TaskProjection>,
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
}

/// Produces the retained record union and its deterministic projections.
pub fn plan_sync(local: &[Record], incoming: &[Record]) -> Result<SyncPlan, String> {
    let retained_records = union(local, incoming)?;
    let task_projections = project_tasks(&retained_records);
    Ok(SyncPlan {
        base_records: local.to_vec(),
        retained_records,
        task_projections,
    })
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
                return Some(TaskProjection::Deleted { entity_id });
            }
            records
                .into_iter()
                .filter_map(|record| match &record.payload {
                    RecordPayload::TaskVersion { title, status } => {
                        Some((record.mutation_time, &record.id, title, *status))
                    }
                    RecordPayload::TaskDeleted => None,
                })
                .max_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
                .map(|(_, record_id, title, status)| TaskProjection::Version {
                    entity_id,
                    record_id: record_id.clone(),
                    title: title.clone(),
                    status,
                })
        })
        .collect()
}

fn validate_records(records: &[Record]) -> Result<(), String> {
    let unique = union(&[], records)?;
    if unique.len() != records.len() {
        return Err("duplicate synchronization record identity".into());
    }
    for record in records {
        if let RecordPayload::TaskVersion { title, .. } = &record.payload {
            pomotui_domain::TaskTitle::parse(title)
                .map_err(|error| format!("invalid synchronized Task: {error}"))?;
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
