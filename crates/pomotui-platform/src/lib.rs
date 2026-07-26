//! Linux implementations of ports defined by Pomotui's inner crates.
//!
//! `SQLite`, suspend-aware clocks, recovery, desktop notification, audio, and
//! service-lifecycle adapters live here.

mod recovery;
mod repository;

pub use recovery::{
    Clock, DesktopReminder, LinuxClock, RecoveryElapsed, RecoveryObservation, RecoverySource,
    ReminderPort, dispatch_reminder, elapsed_during_recovery, observe,
};
pub use repository::{RepositoryError, SqliteRepository};

/// Returns the domain model version targeted by these adapters.
#[must_use]
pub const fn supported_model_version() -> u16 {
    pomotui_domain::MODEL_VERSION
}
