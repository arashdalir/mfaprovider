//! Biometric unlock on top of the vault service (ADR-0003, ADR-0016).

use mfa_core::biometric::{Availability, BiometricError, BiometricProvider};
use mfa_core::vault::{Clock, Rng, Storage};

use super::device_settings::{DeviceSettings, MAX_REPROMPT_DAYS, SettingsStore};
use super::dto::BiometricStatus;
use super::error::{IpcError, IpcErrorCode};
use super::service::VaultService;

impl<S: Storage, C: Clock + Clone, R: Rng> VaultService<S, C, R> {
    /// Uses `provider` for biometric unlock and `store` for this device's settings.
    #[must_use]
    pub fn with_biometrics(
        mut self,
        provider: Box<dyn BiometricProvider>,
        store: Box<dyn SettingsStore>,
    ) -> Self {
        self.biometrics = provider;
        self.device = store;
        self
    }

    /// Needs no unlocked vault: the unlock screen asks it.
    pub fn biometric_status(&self) -> BiometricStatus {
        let settings = self.device.load();
        BiometricStatus {
            available: self.biometrics.availability() == Availability::Available,
            enabled: settings.biometric_enabled,
            password_due: settings.biometric_enabled && settings.password_due(self.clock.unix_ms()),
            reprompt_days: settings.reprompt_days,
        }
    }

    /// Turns biometric unlock on. The device password is asked again: only someone who knows
    /// it may leave a second way into the vault on this device.
    pub fn biometric_enable(&mut self, password: &str) -> Result<(), IpcError> {
        if self.biometrics.availability() != Availability::Available {
            return Err(IpcError::new(IpcErrorCode::BiometricUnavailable));
        }
        let key = self.session.export_key(password)?;
        self.biometrics.enrol(&key).map_err(biometric_error)?;
        let settings = DeviceSettings {
            biometric_enabled: true,
            last_password_ms: self.clock.unix_ms(),
            ..self.device.load()
        };
        if self.device.save(&settings).is_err() {
            self.biometrics.remove();
            return Err(IpcError::new(IpcErrorCode::StorageFailed));
        }
        Ok(())
    }

    /// Turns it off and deletes the device's wrap of the vault key; works while locked.
    pub fn biometric_disable(&mut self) -> Result<(), IpcError> {
        self.switch_off_biometrics()?;
        self.biometrics.remove();
        Ok(())
    }

    /// Unlocks with the biometric. The UI falls back to the device password on any error.
    pub fn unlock_with_biometric(&mut self) -> Result<(), IpcError> {
        let settings = self.device.load();
        if !settings.biometric_enabled {
            return Err(IpcError::new(IpcErrorCode::BiometricNotEnabled));
        }
        if settings.password_due(self.clock.unix_ms()) {
            return Err(IpcError::new(IpcErrorCode::PasswordDue));
        }
        let key = match self.biometrics.release() {
            Ok(key) => key,
            Err(BiometricError::Invalidated) => return Err(self.invalidated()),
            Err(error) => return Err(biometric_error(error)),
        };
        match self.session.unlock_with_key(&key).map_err(IpcError::from) {
            Err(error) if error.code == IpcErrorCode::WrongPassword => Err(self.invalidated()),
            other => other,
        }
    }

    /// How many days biometrics may go without the device password.
    pub fn set_biometric_reprompt_days(&mut self, days: u32) -> Result<(), IpcError> {
        if days == 0 || days > MAX_REPROMPT_DAYS {
            return Err(IpcError::invalid_input(
                "the period must be between 1 and 365 days",
            ));
        }
        let settings = DeviceSettings {
            reprompt_days: days,
            ..self.device.load()
        };
        self.device
            .save(&settings)
            .map_err(|_| IpcError::new(IpcErrorCode::StorageFailed))
    }

    /// Called after the device password was typed correctly.
    pub(super) fn note_password_typed(&mut self) {
        let settings = DeviceSettings {
            last_password_ms: self.clock.unix_ms(),
            ..self.device.load()
        };
        // Best effort: when this cannot be saved the next biometric unlock asks for the
        // password sooner, which is the safe side.
        let _ = self.device.save(&settings);
    }

    fn invalidated(&mut self) -> IpcError {
        let _ = self.switch_off_biometrics();
        self.biometrics.remove();
        IpcError::new(IpcErrorCode::BiometricInvalidated)
    }

    fn switch_off_biometrics(&mut self) -> Result<(), IpcError> {
        let settings = DeviceSettings {
            biometric_enabled: false,
            ..self.device.load()
        };
        self.device
            .save(&settings)
            .map_err(|_| IpcError::new(IpcErrorCode::StorageFailed))
    }
}

fn biometric_error(error: BiometricError) -> IpcError {
    IpcError::new(match error {
        BiometricError::Unavailable => IpcErrorCode::BiometricUnavailable,
        BiometricError::Cancelled => IpcErrorCode::BiometricCancelled,
        BiometricError::Failed => IpcErrorCode::BiometricFailed,
        BiometricError::Invalidated => IpcErrorCode::BiometricInvalidated,
        BiometricError::Platform => IpcErrorCode::BiometricFailed,
    })
}
