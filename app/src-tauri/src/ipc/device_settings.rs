//! Settings that belong to this device, not to the vault: whether biometric unlock is on, when
//! the master password was last typed and how long biometrics may go without it. Kept next to
//! the vault file, never inside it and never synced (ADR-0003, ADR-0008).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

pub const DEFAULT_REPROMPT_DAYS: u32 = 7;
pub const MAX_REPROMPT_DAYS: u32 = 365;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSettings {
    pub biometric_enabled: bool,
    /// Unix milliseconds of the last unlock or enabling with the master password.
    pub last_password_ms: i64,
    pub reprompt_days: u32,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            biometric_enabled: false,
            last_password_ms: 0,
            reprompt_days: DEFAULT_REPROMPT_DAYS,
        }
    }
}

impl DeviceSettings {
    /// True once biometrics has gone `reprompt_days` without the master password.
    #[must_use]
    pub fn password_due(&self, now_ms: i64) -> bool {
        let allowed_ms = i64::from(self.reprompt_days) * 24 * 60 * 60 * 1000;
        now_ms.saturating_sub(self.last_password_ms) > allowed_ms
    }
}

pub trait SettingsStore: Send {
    /// What was saved; defaults when nothing was saved or the file is unreadable (a damaged
    /// file must never keep biometrics switched on).
    fn load(&self) -> DeviceSettings;

    /// # Errors
    /// If the settings cannot be written.
    fn save(&mut self, settings: &DeviceSettings) -> std::io::Result<()>;
}

/// Settings in a small JSON file, replaced atomically.
pub struct FileSettings {
    path: PathBuf,
}

impl FileSettings {
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SettingsStore for FileSettings {
    fn load(&self) -> DeviceSettings {
        std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&mut self, settings: &DeviceSettings) -> std::io::Result<()> {
        let temporary = self.path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec(settings)?)?;
        std::fs::rename(&temporary, &self.path)
    }
}

/// Settings kept in memory; what a store is replaced by in tests and until a file is chosen.
#[derive(Clone, Default)]
pub struct MemorySettings(Arc<Mutex<DeviceSettings>>);

impl SettingsStore for MemorySettings {
    fn load(&self) -> DeviceSettings {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn save(&mut self, settings: &DeviceSettings) -> std::io::Result<()> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = *settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn the_password_is_due_after_the_period_and_not_before() {
        let settings = DeviceSettings {
            last_password_ms: 1_000,
            reprompt_days: 7,
            ..DeviceSettings::default()
        };
        assert!(!settings.password_due(1_000 + 7 * DAY_MS));
        assert!(settings.password_due(1_000 + 7 * DAY_MS + 1));
    }

    #[test]
    fn a_clock_that_went_backwards_does_not_make_the_password_due() {
        let settings = DeviceSettings {
            last_password_ms: 10 * DAY_MS,
            ..DeviceSettings::default()
        };
        assert!(!settings.password_due(0));
    }

    #[test]
    fn settings_survive_a_round_trip_through_the_file() {
        let directory = std::env::temp_dir().join(format!("mfa-device-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut store = FileSettings::new(directory.join("device.json"));
        assert_eq!(store.load(), DeviceSettings::default());
        let wanted = DeviceSettings {
            biometric_enabled: true,
            last_password_ms: 42,
            reprompt_days: 3,
        };
        store.save(&wanted).unwrap();
        assert_eq!(store.load(), wanted);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_damaged_file_means_defaults_so_biometrics_stays_off() {
        let directory = std::env::temp_dir().join(format!("mfa-device-bad-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("device.json");
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(!FileSettings::new(path).load().biometric_enabled);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
