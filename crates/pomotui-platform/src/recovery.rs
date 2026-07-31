#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryObservation {
    pub boot_id: String,
    pub monotonic_seconds: u64,
    pub wall_seconds: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoverySource {
    SameBootMonotonic,
    NewBootWallEstimate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryElapsed {
    pub seconds: u64,
    pub source: RecoverySource,
}

#[must_use]
pub fn elapsed_during_recovery(
    persisted: &RecoveryObservation,
    current: &RecoveryObservation,
) -> RecoveryElapsed {
    if persisted.boot_id == current.boot_id {
        RecoveryElapsed {
            seconds: current
                .monotonic_seconds
                .saturating_sub(persisted.monotonic_seconds),
            source: RecoverySource::SameBootMonotonic,
        }
    } else {
        RecoveryElapsed {
            seconds: current
                .wall_seconds
                .saturating_sub(persisted.wall_seconds)
                .try_into()
                .unwrap_or(0),
            source: RecoverySource::NewBootWallEstimate,
        }
    }
}

pub trait ReminderPort {
    type Error;

    /// Emits a desktop notification.
    ///
    /// # Errors
    ///
    /// Returns an adapter-specific delivery error.
    fn notify(&mut self) -> Result<(), Self::Error>;
    /// Plays the configured sound.
    ///
    /// # Errors
    ///
    /// Returns an adapter-specific playback error.
    fn play_sound(&mut self) -> Result<(), Self::Error>;
}

pub fn dispatch_reminder<P: ReminderPort>(port: &mut P) {
    let _notification_result = port.notify();
    let _sound_result = port.play_sound();
}

#[derive(Clone, Debug)]
pub struct DesktopReminder {
    pub sound: Option<std::path::PathBuf>,
    pub volume_percent: u8,
}

impl ReminderPort for DesktopReminder {
    type Error = std::io::Error;

    fn notify(&mut self) -> Result<(), Self::Error> {
        let status = std::process::Command::new("notify-send")
            .args(["Pomotui", "Session complete"])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("notify-send failed"))
        }
    }

    fn play_sound(&mut self) -> Result<(), Self::Error> {
        let Some(sound) = &self.sound else {
            return Ok(());
        };
        let pulse_volume = u32::from(self.volume_percent).min(100) * 65_536 / 100;
        let status = std::process::Command::new("paplay")
            .arg(format!("--volume={pulse_volume}"))
            .arg(sound)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("paplay failed"))
        }
    }
}

pub trait Clock {
    type Error;

    /// Reads Linux suspend-aware elapsed seconds.
    ///
    /// # Errors
    ///
    /// Returns an adapter error if the clock cannot be observed.
    fn monotonic_seconds(&self) -> Result<u64, Self::Error>;

    /// Reads the Unix wall clock for reboot recovery only.
    ///
    /// # Errors
    ///
    /// Returns an adapter error if the clock cannot be observed.
    fn wall_seconds(&self) -> Result<i64, Self::Error>;

    /// Reads the current kernel boot identity.
    ///
    /// # Errors
    ///
    /// Returns an adapter error if the boot identity cannot be read.
    fn boot_id(&self) -> Result<String, Self::Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxClock;

impl Clock for LinuxClock {
    type Error = std::io::Error;

    fn monotonic_seconds(&self) -> Result<u64, Self::Error> {
        let uptime = std::fs::read_to_string("/proc/uptime")?;
        let value = uptime
            .split_whitespace()
            .next()
            .ok_or_else(|| std::io::Error::other("missing /proc/uptime value"))?;
        let seconds = value
            .split('.')
            .next()
            .ok_or_else(|| std::io::Error::other("invalid /proc/uptime value"))?;
        seconds
            .parse()
            .map_err(|error| std::io::Error::other(format!("invalid uptime: {error}")))
    }

    fn wall_seconds(&self) -> Result<i64, Self::Error> {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_secs();
        i64::try_from(seconds).map_err(std::io::Error::other)
    }

    fn boot_id(&self) -> Result<String, Self::Error> {
        Ok(std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?
            .trim()
            .to_owned())
    }
}

#[cfg(target_os = "macos")]
mod darwin {
    /// Reads boot time via `sysctl -n kern.boottime` (returns `{ sec, usec }`).
    fn boot_epoch_secs() -> Result<i64, std::io::Error> {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "kern.boottime"])
            .output()?;
        if !output.status.success() {
            return Err(std::io::Error::other("sysctl kern.boottime failed"));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        // Output looks like: { sec = 1234567890, usec = 123456 }
        let sec_str = text
            .split("sec = ")
            .nth(1)
            .and_then(|s| s.split(',').next())
            .ok_or_else(|| std::io::Error::other("unexpected kern.boottime format"))?;
        sec_str
            .trim()
            .parse::<i64>()
            .map_err(|e| std::io::Error::other(format!("invalid kern.boottime: {e}")))
    }

    /// Returns suspend-aware uptime in whole seconds.
    pub fn uptime_seconds() -> Result<u64, std::io::Error> {
        let boot = boot_epoch_secs()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_secs();
        let now_i64 = i64::try_from(now).map_err(std::io::Error::other)?;
        Ok(u64::try_from(now_i64.saturating_sub(boot)).unwrap_or(0))
    }

    /// Reads the platform UUID via `ioreg`.
    pub fn platform_uuid() -> Result<String, std::io::Error> {
        let output = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()?;
        if !output.status.success() {
            return Err(std::io::Error::other("ioreg failed"));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("IOPlatformUUID") {
                // Line looks like: "IOPlatformUUID" = "XXXXXXXX-XXXX-..."
                if let Some(uuid) = line.split('"').nth(3) {
                    return Ok(uuid.to_owned());
                }
            }
        }
        Err(std::io::Error::other("IOPlatformUUID not found"))
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
pub struct DarwinClock;

#[cfg(target_os = "macos")]
impl Clock for DarwinClock {
    type Error = std::io::Error;

    fn monotonic_seconds(&self) -> Result<u64, Self::Error> {
        darwin::uptime_seconds()
    }

    fn wall_seconds(&self) -> Result<i64, Self::Error> {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_secs();
        i64::try_from(seconds).map_err(std::io::Error::other)
    }

    fn boot_id(&self) -> Result<String, Self::Error> {
        darwin::platform_uuid()
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
pub struct MacDesktopReminder {
    pub sound: Option<std::path::PathBuf>,
    pub volume_percent: u8,
}

#[cfg(target_os = "macos")]
impl ReminderPort for MacDesktopReminder {
    type Error = std::io::Error;

    fn notify(&mut self) -> Result<(), Self::Error> {
        let status = std::process::Command::new("osascript")
            .args([
                "-e",
                "display notification \"Session complete\" with title \"Pomotui\"",
            ])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("osascript notification failed"))
        }
    }

    fn play_sound(&mut self) -> Result<(), Self::Error> {
        let Some(sound) = &self.sound else {
            return Ok(());
        };
        // afplay volume range is 0.0–1.0
        let volume = f64::from(self.volume_percent.min(100)) / 100.0;
        let status = std::process::Command::new("afplay")
            .arg(format!("--volume={volume}"))
            .arg(sound)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("afplay failed"))
        }
    }
}

/// Captures one durable recovery observation.
///
/// # Errors
///
/// Returns the first clock-adapter error.
pub fn observe(clock: &impl Clock<Error = std::io::Error>) -> std::io::Result<RecoveryObservation> {
    Ok(RecoveryObservation {
        boot_id: clock.boot_id()?,
        monotonic_seconds: clock.monotonic_seconds()?,
        wall_seconds: clock.wall_seconds()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(boot: &str, monotonic: u64, wall: i64) -> RecoveryObservation {
        RecoveryObservation {
            boot_id: boot.into(),
            monotonic_seconds: monotonic,
            wall_seconds: wall,
        }
    }

    #[test]
    fn same_boot_ignores_wall_clock_changes() {
        let elapsed =
            elapsed_during_recovery(&observation("a", 10, 5_000), &observation("a", 25, -9_000));
        assert_eq!(elapsed.seconds, 15);
        assert_eq!(elapsed.source, RecoverySource::SameBootMonotonic);
    }

    #[test]
    fn reboot_uses_wall_estimate_and_clamps_backwards_change() {
        assert_eq!(
            elapsed_during_recovery(&observation("a", 10, 100), &observation("b", 2, 90)).seconds,
            0
        );
        assert_eq!(
            elapsed_during_recovery(&observation("a", 10, 100), &observation("b", 2, 130)).seconds,
            30
        );
    }

    struct FailingReminder {
        attempts: u8,
    }

    impl ReminderPort for FailingReminder {
        type Error = &'static str;

        fn notify(&mut self) -> Result<(), Self::Error> {
            self.attempts += 1;
            Err("notification failed")
        }

        fn play_sound(&mut self) -> Result<(), Self::Error> {
            self.attempts += 1;
            Err("sound failed")
        }
    }

    #[test]
    fn external_effect_failures_are_isolated_and_both_are_attempted() {
        let mut reminder = FailingReminder { attempts: 0 };
        dispatch_reminder(&mut reminder);
        assert_eq!(reminder.attempts, 2);
    }
}
