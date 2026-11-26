//! The Android Keystore as a [`BiometricProvider`] (ADR-0016), reached through the Kotlin
//! plugin `KeystorePlugin`. The vault key travels as base64 over the plugin bridge, in memory
//! only; the Kotlin side keeps it wrapped by a key that only a fresh biometric can use.

// Only Android calls the bridge; the pure parts are still unit-tested on every platform.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mfa_core::biometric::{BiometricError, VaultKeyBytes};
use mfa_core::vault::VAULT_KEY_LENGTH;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// What the Kotlin plugin rejects with, as the `code` of its error.
fn error_from_code(code: Option<&str>) -> BiometricError {
    match code {
        Some("unavailable") => BiometricError::Unavailable,
        Some("cancelled") => BiometricError::Cancelled,
        Some("failed") => BiometricError::Failed,
        Some("invalidated") => BiometricError::Invalidated,
        _ => BiometricError::Platform,
    }
}

fn key_from_base64(encoded: String) -> Result<VaultKeyBytes, BiometricError> {
    let encoded = Zeroizing::new(encoded);
    let bytes = Zeroizing::new(
        STANDARD
            .decode(encoded.as_bytes())
            .map_err(|_| BiometricError::Platform)?,
    );
    let key: [u8; VAULT_KEY_LENGTH] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| BiometricError::Platform)?;
    Ok(Zeroizing::new(key))
}

#[derive(Deserialize)]
struct AvailabilityReply {
    available: bool,
}

#[derive(Deserialize)]
struct ReleasedKey {
    key: String,
}

#[derive(Serialize)]
struct KeyToWrap<'a> {
    key: &'a str,
}

#[cfg(target_os = "android")]
pub use android::{AndroidBiometrics, init};

#[cfg(target_os = "android")]
mod android {
    use mfa_core::biometric::{Availability, BiometricProvider};
    use serde::de::IgnoredAny;
    use tauri::plugin::{PluginHandle, TauriPlugin};
    use tauri::{Manager, Runtime};

    use super::*;

    const ANDROID_PACKAGE: &str = "dev.area51.android.mfaprovider";
    const PLUGIN_CLASS: &str = "KeystorePlugin";

    struct Handle<R: Runtime>(PluginHandle<R>);

    /// Registers the Kotlin plugin; must be added to the builder.
    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        tauri::plugin::Builder::new("keystore")
            .setup(|app, api| {
                let handle = api.register_android_plugin(ANDROID_PACKAGE, PLUGIN_CLASS)?;
                app.manage(Handle(handle));
                Ok(())
            })
            .build()
    }

    pub struct AndroidBiometrics<R: Runtime>(PluginHandle<R>);

    impl<R: Runtime> AndroidBiometrics<R> {
        pub fn from_app(app: &tauri::AppHandle<R>) -> Self {
            Self(app.state::<Handle<R>>().0.clone())
        }

        fn call<T: serde::de::DeserializeOwned>(
            &self,
            command: &str,
            payload: impl Serialize,
        ) -> Result<T, BiometricError> {
            self.0
                .run_mobile_plugin(command, payload)
                .map_err(|failure| match failure {
                    tauri::plugin::mobile::PluginInvokeError::InvokeRejected(rejection) => {
                        error_from_code(rejection.code.as_deref())
                    }
                    _ => BiometricError::Platform,
                })
        }
    }

    impl<R: Runtime> BiometricProvider for AndroidBiometrics<R> {
        fn availability(&self) -> Availability {
            match self.call::<AvailabilityReply>("availability", ()) {
                Ok(AvailabilityReply { available: true }) => Availability::Available,
                _ => Availability::Unavailable,
            }
        }

        fn enrol(&mut self, key: &[u8; VAULT_KEY_LENGTH]) -> Result<(), BiometricError> {
            let encoded = Zeroizing::new(STANDARD.encode(key));
            self.call::<IgnoredAny>("enrol", KeyToWrap { key: &encoded })
                .map(|_| ())
        }

        fn release(&mut self) -> Result<VaultKeyBytes, BiometricError> {
            let released = self.call::<ReleasedKey>("release", ())?;
            key_from_base64(released.key)
        }

        fn remove(&mut self) {
            let _ = self.call::<IgnoredAny>("remove", ());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_the_plugin_sends_has_its_error() {
        assert_eq!(
            error_from_code(Some("cancelled")),
            BiometricError::Cancelled
        );
        assert_eq!(error_from_code(Some("failed")), BiometricError::Failed);
        assert_eq!(
            error_from_code(Some("invalidated")),
            BiometricError::Invalidated
        );
        assert_eq!(
            error_from_code(Some("unavailable")),
            BiometricError::Unavailable
        );
    }

    #[test]
    fn an_unknown_or_missing_code_is_a_platform_failure() {
        assert_eq!(error_from_code(Some("boom")), BiometricError::Platform);
        assert_eq!(error_from_code(None), BiometricError::Platform);
    }

    #[test]
    fn a_released_key_must_be_exactly_the_vault_key_length() {
        let good = STANDARD.encode([7u8; VAULT_KEY_LENGTH]);
        assert_eq!(*key_from_base64(good).unwrap(), [7u8; VAULT_KEY_LENGTH]);
        let short = STANDARD.encode([7u8; 5]);
        assert_eq!(
            key_from_base64(short).unwrap_err(),
            BiometricError::Platform
        );
        assert_eq!(
            key_from_base64("not base64!".into()).unwrap_err(),
            BiometricError::Platform
        );
    }
}
