use rusqlite::{Connection, OptionalExtension, Transaction};
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

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
                PRAGMA user_version = 1;
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
    ) -> Result<bool, RepositoryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO current_session(singleton, payload) VALUES (1, ?1)
             ON CONFLICT(singleton) DO UPDATE SET payload = excluded.payload",
            [payload],
        )?;
        let claimed = transaction.execute(
            "INSERT OR IGNORE INTO reminders(session_key, emitted) VALUES (?1, 1)",
            [session_key],
        )? == 1;
        transaction.commit()?;
        Ok(claimed)
    }

    #[cfg(test)]
    fn set_user_version(&self, version: i64) -> rusqlite::Result<()> {
        self.connection.pragma_update(None, "user_version", version)
    }
}

#[cfg(test)]
mod tests {
    use super::{RepositoryError, SqliteRepository};
    use rusqlite::Connection;

    #[test]
    fn migration_creates_complete_v1_schema() {
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
                "current_session",
                "focus_cycle",
                "mutation_keys",
                "recovery_observations",
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
                supported: 1
            }
        ));
    }

    #[test]
    fn schema_version_is_one() {
        let repository = SqliteRepository::open_in_memory().expect("open");
        repository.set_user_version(1).expect("set");
        assert_eq!(SqliteRepository::schema_version(), 1);
    }

    #[test]
    fn reminder_is_claimed_only_once() {
        let mut repository = SqliteRepository::open_in_memory().expect("open");
        assert!(repository.claim_reminder("session-7").expect("first"));
        assert!(!repository.claim_reminder("session-7").expect("restart"));
    }
}
