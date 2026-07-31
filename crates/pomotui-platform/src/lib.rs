//! Platform-specific implementations of ports defined by Pomotui's inner crates.
//!
//! `SQLite`, suspend-aware clocks, recovery, desktop notification, audio, and
//! service-lifecycle adapters live here.

mod recovery;
mod repository;

pub use recovery::{
    Clock, DesktopReminder, LinuxClock, RecoveryElapsed, RecoveryObservation, RecoverySource,
    ReminderPort, dispatch_reminder, elapsed_during_recovery, observe,
};

#[cfg(target_os = "macos")]
pub use recovery::{DarwinClock, MacDesktopReminder};

/// The platform-appropriate clock for the current OS.
#[cfg(target_os = "macos")]
pub type PlatformClock = DarwinClock;

/// The platform-appropriate clock for the current OS.
#[cfg(not(target_os = "macos"))]
pub type PlatformClock = LinuxClock;

/// The platform-appropriate desktop reminder for the current OS.
#[cfg(target_os = "macos")]
pub type PlatformDesktopReminder = MacDesktopReminder;

/// The platform-appropriate desktop reminder for the current OS.
#[cfg(not(target_os = "macos"))]
pub type PlatformDesktopReminder = DesktopReminder;

pub use repository::{
    PendingReminderEffect, ReminderDeliveryCounts, ReminderEffectKind, RepositoryError,
    SqliteRepository,
};

/// Returns the domain model version targeted by these adapters.
#[must_use]
pub const fn supported_model_version() -> u16 {
    pomotui_domain::MODEL_VERSION
}
