use rusqlite::{Connection, OptionalExtension, Transaction};
use std::path::Path;

const SCHEMA_VERSION: i64 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReminderEffectKind {
    Notification,
    Sound,
}

impl ReminderEffectKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Notification => "notification",
            Self::Sound => "sound",
        }
    }

    fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "notification" => Ok(Self::Notification),
            "sound" => Ok(Self::Sound),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingReminderEffect {
    pub id: i64,
    pub session_key: String,
    pub kind: ReminderEffectKind,
    pub attempt_count: u32,
    pub created_at: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReminderDeliveryCounts {
    pub pending: u32,
    pub retrying: u32,
    pub delivered: u32,
    pub exhausted: u32,
}

#[derive(Debug)]
pub enum RepositoryError {
    Sqlite(rusqlite::Error),
    IncompatibleSchema { found: i64, supported: i64 },
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite repository error: {error}"),
            Self::IncompatibleSchema { found, supported } => {
                write!(
                    formatter,
                    "database schema {found} is newer than supported schema {supported}"
                )
            }
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<rusqlite::Error> for RepositoryError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

pub struct SqliteRepository {
    connection: Connection,
}

impl SqliteRepository {
    /// Opens and migrates a repository at `path`.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic error when `SQLite` cannot open/migrate the file or
    /// when its schema is newer than this binary supports.
    pub fn open(path: &Path) -> Result<Self, RepositoryError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens a migrated in-memory repository.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic `SQLite` migration error.
    pub fn open_in_memory() -> Result<Self, RepositoryError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    #[allow(clippy::too_many_lines)]
    fn from_connection(mut connection: Connection) -> Result<Self, RepositoryError> {
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let found = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if found > SCHEMA_VERSION {
            return Err(RepositoryError::IncompatibleSchema {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        if found == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "
                CREATE TABLE current_session (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    payload TEXT NOT NULL
                );
                CREATE TABLE focus_cycle (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    completed_rounds INTEGER NOT NULL,
                    rounds_per_cycle INTEGER NOT NULL
                );
                CREATE TABLE tasks (
                    id INTEGER PRIMARY KEY,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL CHECK (status IN ('open', 'completed'))
                );
                CREATE TABLE session_history (
                    id INTEGER PRIMARY KEY,
                    ended_at INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    outcome TEXT NOT NULL,
                    planned_seconds INTEGER NOT NULL,
                    actual_seconds INTEGER NOT NULL,
                    task_id INTEGER,
                    task_title TEXT
                );
                CREATE TABLE recovery_observations (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    boot_id TEXT NOT NULL,
                    monotonic_seconds INTEGER NOT NULL,
                    wall_seconds INTEGER NOT NULL
                );
                CREATE TABLE mutation_keys (
                    key TEXT PRIMARY KEY,
                    applied_at INTEGER NOT NULL DEFAULT (unixepoch())
                );
                CREATE TABLE reminders (
                    session_key TEXT PRIMARY KEY,
                    emitted INTEGER NOT NULL CHECK (emitted IN (0, 1))
                );
                CREATE TABLE reminder_outbox (
                    id INTEGER PRIMARY KEY,
                    session_key TEXT NOT NULL,
                    effect_kind TEXT NOT NULL
                        CHECK (effect_kind IN ('notification', 'sound')),
                    state TEXT NOT NULL DEFAULT 'pending'
                        CHECK (state IN ('pending', 'delivered', 'exhausted')),
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    next_attempt_at INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    acknowledged_at INTEGER,
                    last_error TEXT,
                    UNIQUE(session_key, effect_kind)
                );
                CREATE TABLE action_chains (
                    id INTEGER PRIMARY KEY,
                    state TEXT NOT NULL CHECK (state IN ('current', 'ended')),
                    link_count INTEGER NOT NULL DEFAULT 0 CHECK (link_count >= 0)
                );
                CREATE UNIQUE INDEX one_current_action_chain
                    ON action_chains(state) WHERE state = 'current';
                INSERT INTO action_chains(id, state, link_count)
                    VALUES (1, 'current', 0);
                CREATE TABLE pending_reviews (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    session_id INTEGER NOT NULL UNIQUE,
                    actual_seconds INTEGER NOT NULL,
                    task_id INTEGER,
                    task_title TEXT
                );
                PRAGMA user_version = 3;
                ",
            )?;
            transaction.commit()?;
        } else if found == 1 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "
                CREATE TABLE reminder_outbox (
                    id INTEGER PRIMARY KEY,
                    session_key TEXT NOT NULL,
                    effect_kind TEXT NOT NULL
                        CHECK (effect_kind IN ('notification', 'sound')),
                    state TEXT NOT NULL DEFAULT 'pending'
                        CHECK (state IN ('pending', 'delivered', 'exhausted')),
                    attempt_count INTEGER NOT NULL DEFAULT 0,
                    next_attempt_at INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    acknowledged_at INTEGER,
                    last_error TEXT,
                    UNIQUE(session_key, effect_kind)
                );
                CREATE TABLE action_chains (
                    id INTEGER PRIMARY KEY,
                    state TEXT NOT NULL CHECK (state IN ('current', 'ended')),
                    link_count INTEGER NOT NULL DEFAULT 0 CHECK (link_count >= 0)
                );
                CREATE UNIQUE INDEX one_current_action_chain
                    ON action_chains(state) WHERE state = 'current';
                INSERT INTO action_chains(id, state, link_count)
                    VALUES (1, 'current', 0);
                CREATE TABLE pending_reviews (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    session_id INTEGER NOT NULL UNIQUE,
                    actual_seconds INTEGER NOT NULL,
                    task_id INTEGER,
                    task_title TEXT
                );
                PRAGMA user_version = 3;
                ",
            )?;
            transaction.commit()?;
        } else if found == 2 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "
                CREATE TABLE action_chains (
                    id INTEGER PRIMARY KEY,
                    state TEXT NOT NULL CHECK (state IN ('current', 'ended')),
                    link_count INTEGER NOT NULL DEFAULT 0 CHECK (link_count >= 0)
                );
                CREATE UNIQUE INDEX one_current_action_chain
                    ON action_chains(state) WHERE state = 'current';
                INSERT INTO action_chains(id, state, link_count)
                    VALUES (1, 'current', 0);
                CREATE TABLE pending_reviews (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    session_id INTEGER NOT NULL UNIQUE,
                    actual_seconds INTEGER NOT NULL,
                    task_id INTEGER,
                    task_title TEXT
                );
                PRAGMA user_version = 3;
                ",
            )?;
            transaction.commit()?;
        }
        Ok(Self { connection })
    }

    #[must_use]
    pub const fn schema_version() -> i64 {
        SCHEMA_VERSION
    }

    /// Applies a mutation atomically at most once.
    ///
    /// The closure is not called when `key` has already committed.
    ///
    /// # Errors
    ///
    /// Returns the closure's `SQLite` error or a transaction error. Failed
    /// closures roll back both their writes and the idempotency key.
    pub fn apply_once(
        &mut self,
        key: &str,
        operation: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<()>,
    ) -> Result<bool, RepositoryError> {
        let transaction = self.connection.transaction()?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO mutation_keys(key) VALUES (?1)",
            [key],
        )?;
        if inserted == 0 {
            return Ok(false);
        }
        operation(&transaction)?;
        transaction.commit()?;
        Ok(true)
    }

    /// Reads the opaque Current Session representation.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` read error.
    pub fn current_session_payload(&self) -> Result<Option<String>, RepositoryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT payload FROM current_session WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Atomically replaces the durable service snapshot.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` transaction error.
    pub fn save_state(&mut self, payload: &str) -> Result<(), RepositoryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO current_session(singleton, payload) VALUES (1, ?1)
             ON CONFLICT(singleton) DO UPDATE SET payload = excluded.payload",
            [payload],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Atomically saves state and commits a mutation identity at most once.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` transaction error.
    pub fn save_state_once(&mut self, key: &str, payload: &str) -> Result<bool, RepositoryError> {
        self.apply_once(key, |transaction| {
            transaction.execute(
                "INSERT INTO current_session(singleton, payload) VALUES (1, ?1)
                 ON CONFLICT(singleton) DO UPDATE SET payload = excluded.payload",
                [payload],
            )?;
            Ok(())
        })
    }

    /// Returns all committed mutation identities.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` read error.
    pub fn mutation_keys(&self) -> Result<Vec<String>, RepositoryError> {
        let mut statement = self
            .connection
            .prepare("SELECT key FROM mutation_keys ORDER BY key")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Atomically claims reminder delivery for a completed Session.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` transaction error. A `false` result means another
    /// service incarnation already claimed this reminder.
    pub fn claim_reminder(&mut self, session_key: &str) -> Result<bool, RepositoryError> {
        let changed = self.connection.execute(
            "INSERT OR IGNORE INTO reminders(session_key, emitted) VALUES (?1, 1)",
            [session_key],
        )?;
        Ok(changed == 1)
    }

    /// Atomically persists a completed transition and claims its reminder.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` transaction error.
    pub fn save_completion(
        &mut self,
        payload: &str,
        session_key: &str,
        effects: &[ReminderEffectKind],
        created_at: i64,
    ) -> Result<bool, RepositoryError> {
        let transaction = self.connection.transaction()?;
        let claimed = transaction.execute(
            "INSERT OR IGNORE INTO reminders(session_key, emitted) VALUES (?1, 1)",
            [session_key],
        )? == 1;
        if claimed {
            transaction.execute(
                "INSERT INTO current_session(singleton, payload) VALUES (1, ?1)
                 ON CONFLICT(singleton) DO UPDATE SET payload = excluded.payload",
                [payload],
            )?;
            for effect in effects {
                transaction.execute(
                    "INSERT INTO reminder_outbox(
                         session_key, effect_kind, next_attempt_at, created_at
                     ) VALUES (?1, ?2, ?3, ?3)",
                    rusqlite::params![session_key, effect.as_str(), created_at],
                )?;
            }
        }
        transaction.commit()?;
        Ok(claimed)
    }

    /// Reads all Session Reminder effects awaiting delivery.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` query or data-conversion error.
    pub fn pending_reminder_effects(&self) -> Result<Vec<PendingReminderEffect>, RepositoryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, session_key, effect_kind, attempt_count, created_at
             FROM reminder_outbox
             WHERE state = 'pending'
             ORDER BY id",
        )?;
        Ok(statement
            .query_map([], |row| {
                Ok(PendingReminderEffect {
                    id: row.get(0)?,
                    session_key: row.get(1)?,
                    kind: ReminderEffectKind::parse(&row.get::<_, String>(2)?)?,
                    attempt_count: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    /// Reads Session Reminder effects whose delivery time has arrived.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` query or data-conversion error.
    pub fn due_reminder_effects(
        &self,
        now: i64,
    ) -> Result<Vec<PendingReminderEffect>, RepositoryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, session_key, effect_kind, attempt_count, created_at
             FROM reminder_outbox
             WHERE state = 'pending' AND next_attempt_at <= ?1
             ORDER BY id",
        )?;
        Ok(statement
            .query_map([now], |row| {
                Ok(PendingReminderEffect {
                    id: row.get(0)?,
                    session_key: row.get(1)?,
                    kind: ReminderEffectKind::parse(&row.get::<_, String>(2)?)?,
                    attempt_count: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    /// Records a failed Session Reminder attempt and either reschedules or
    /// exhausts it.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` write error.
    pub fn record_reminder_failure(
        &mut self,
        id: i64,
        failed_at: i64,
        next_attempt_at: i64,
        exhausted: bool,
        error: &str,
    ) -> Result<(), RepositoryError> {
        self.connection.execute(
            "UPDATE reminder_outbox
             SET state = CASE WHEN ?4 THEN 'exhausted' ELSE 'pending' END,
                 attempt_count = attempt_count + 1,
                 next_attempt_at = ?3,
                 acknowledged_at = CASE WHEN ?4 THEN ?2 ELSE NULL END,
                 last_error = ?5
             WHERE id = ?1 AND state = 'pending'",
            rusqlite::params![id, failed_at, next_attempt_at, exhausted, error],
        )?;
        Ok(())
    }

    /// Returns aggregate Session Reminder delivery states.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` query error.
    pub fn reminder_delivery_counts(&self) -> Result<ReminderDeliveryCounts, RepositoryError> {
        Ok(self.connection.query_row(
            "SELECT
                 COALESCE(SUM(state = 'pending' AND attempt_count = 0), 0),
                 COALESCE(SUM(state = 'pending' AND attempt_count > 0), 0),
                 COALESCE(SUM(state = 'delivered'), 0),
                 COALESCE(SUM(state = 'exhausted'), 0)
             FROM reminder_outbox",
            [],
            |row| {
                Ok(ReminderDeliveryCounts {
                    pending: row.get(0)?,
                    retrying: row.get(1)?,
                    delivered: row.get(2)?,
                    exhausted: row.get(3)?,
                })
            },
        )?)
    }

    /// Marks one Session Reminder effect as delivered.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` write error.
    pub fn acknowledge_reminder_effect(
        &mut self,
        id: i64,
        acknowledged_at: i64,
    ) -> Result<(), RepositoryError> {
        self.connection.execute(
            "UPDATE reminder_outbox
             SET state = 'delivered', acknowledged_at = ?2
             WHERE id = ?1 AND state = 'pending'",
            rusqlite::params![id, acknowledged_at],
        )?;
        Ok(())
    }

    #[cfg(test)]
    fn set_user_version(&self, version: i64) -> rusqlite::Result<()> {
        self.connection.pragma_update(None, "user_version", version)
    }
}

#[cfg(test)]
mod tests {
    use super::{ReminderDeliveryCounts, ReminderEffectKind, RepositoryError, SqliteRepository};
    use rusqlite::Connection;

    #[test]
    fn migration_creates_complete_v3_schema() {
        let repository = SqliteRepository::open_in_memory().expect("open");
        let names: Vec<String> = repository
            .connection
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        assert_eq!(
            names,
            [
                "action_chains",
                "current_session",
                "focus_cycle",
                "mutation_keys",
                "pending_reviews",
                "recovery_observations",
                "reminder_outbox",
                "reminders",
                "session_history",
                "tasks",
            ]
        );
    }

    #[test]
    fn mutation_is_atomic_and_idempotent() {
        let mut repository = SqliteRepository::open_in_memory().expect("open");
        let write = |transaction: &rusqlite::Transaction<'_>| {
            transaction.execute(
                "INSERT INTO current_session(singleton, payload) VALUES (1, 'running')",
                [],
            )?;
            Ok(())
        };
        assert!(repository.apply_once("start-1", write).expect("first"));
        assert!(
            !repository
                .apply_once("start-1", |_| Ok(()))
                .expect("replay")
        );
        assert_eq!(
            repository
                .current_session_payload()
                .expect("read")
                .as_deref(),
            Some("running")
        );
    }

    #[test]
    fn failed_mutation_rolls_back_key_and_domain_write() {
        let mut repository = SqliteRepository::open_in_memory().expect("open");
        let result = repository.apply_once("bad", |transaction| {
            transaction.execute(
                "INSERT INTO current_session(singleton, payload) VALUES (1, 'partial')",
                [],
            )?;
            Err(rusqlite::Error::InvalidQuery)
        });
        assert!(result.is_err());
        assert_eq!(repository.current_session_payload().expect("read"), None);
        assert!(
            repository
                .apply_once("bad", |_| Ok(()))
                .expect("key rolled back")
        );
    }

    #[test]
    fn newer_schema_fails_without_recreating_data() {
        let connection = Connection::open_in_memory().expect("open");
        connection
            .execute("CREATE TABLE precious(value TEXT)", [])
            .expect("table");
        connection
            .execute("INSERT INTO precious VALUES ('keep')", [])
            .expect("data");
        connection
            .pragma_update(None, "user_version", 99)
            .expect("version");

        let error = SqliteRepository::from_connection(connection)
            .err()
            .expect("reject");
        assert!(matches!(
            error,
            RepositoryError::IncompatibleSchema {
                found: 99,
                supported: 3
            }
        ));
    }

    #[test]
    fn schema_version_is_three() {
        let repository = SqliteRepository::open_in_memory().expect("open");
        repository.set_user_version(3).expect("set");
        assert_eq!(SqliteRepository::schema_version(), 3);
    }

    #[test]
    fn version_one_database_migrates_without_recreating_existing_data() {
        let connection = Connection::open_in_memory().expect("open");
        connection
            .execute("CREATE TABLE precious(value TEXT)", [])
            .expect("table");
        connection
            .execute("INSERT INTO precious VALUES ('keep')", [])
            .expect("data");
        connection
            .pragma_update(None, "user_version", 1)
            .expect("version");

        let repository = SqliteRepository::from_connection(connection).expect("migrate");
        let value: String = repository
            .connection
            .query_row("SELECT value FROM precious", [], |row| row.get(0))
            .expect("preserved data");
        let version: i64 = repository
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("schema version");
        assert_eq!(value, "keep");
        assert_eq!(version, 3);
    }

    #[test]
    fn reminder_is_claimed_only_once() {
        let mut repository = SqliteRepository::open_in_memory().expect("open");
        assert!(repository.claim_reminder("session-7").expect("first"));
        assert!(!repository.claim_reminder("session-7").expect("restart"));
    }

    #[test]
    fn completion_atomically_creates_independent_reminder_effects() {
        let mut repository = SqliteRepository::open_in_memory().expect("open");
        assert!(
            repository
                .save_completion(
                    "completed",
                    "session-8",
                    &[ReminderEffectKind::Notification, ReminderEffectKind::Sound,],
                    100,
                )
                .expect("completion")
        );
        assert!(
            !repository
                .save_completion(
                    "duplicate",
                    "session-8",
                    &[ReminderEffectKind::Notification],
                    101,
                )
                .expect("duplicate completion")
        );

        let effects = repository.pending_reminder_effects().expect("outbox");
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].session_key, "session-8");
        assert_eq!(effects[0].kind, ReminderEffectKind::Notification);
        assert_eq!(effects[1].kind, ReminderEffectKind::Sound);
        assert_eq!(
            repository
                .current_session_payload()
                .expect("state")
                .as_deref(),
            Some("completed")
        );

        repository
            .acknowledge_reminder_effect(effects[0].id, 102)
            .expect("acknowledge notification");
        let remaining = repository.pending_reminder_effects().expect("remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].kind, ReminderEffectKind::Sound);
    }

    #[test]
    fn reminder_failures_retry_when_due_and_eventually_exhaust() {
        let mut repository = SqliteRepository::open_in_memory().expect("open");
        repository
            .save_completion(
                "completed",
                "session-9",
                &[ReminderEffectKind::Notification],
                100,
            )
            .expect("completion");
        let effect = repository
            .due_reminder_effects(100)
            .expect("initial due")
            .pop()
            .expect("effect");

        repository
            .record_reminder_failure(effect.id, 100, 110, false, "temporary")
            .expect("first failure");
        assert!(
            repository
                .due_reminder_effects(109)
                .expect("not due")
                .is_empty()
        );
        assert_eq!(repository.due_reminder_effects(110).expect("due").len(), 1);
        assert_eq!(
            repository.reminder_delivery_counts().expect("counts"),
            ReminderDeliveryCounts {
                pending: 0,
                retrying: 1,
                delivered: 0,
                exhausted: 0,
            }
        );

        repository
            .record_reminder_failure(effect.id, 110, 130, true, "permanent")
            .expect("exhaust");
        assert!(
            repository
                .due_reminder_effects(1_000)
                .expect("terminal")
                .is_empty()
        );
        assert_eq!(
            repository.reminder_delivery_counts().expect("counts"),
            ReminderDeliveryCounts {
                pending: 0,
                retrying: 0,
                delivered: 0,
                exhausted: 1,
            }
        );
    }
}
