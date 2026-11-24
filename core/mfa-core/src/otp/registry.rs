use std::collections::HashMap;

use thiserror::Error;

use super::hotp::Hotp;
use super::otp_type::OtpType;
use super::steam::Steam;
use super::totp::Totp;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("an OTP type with id `{0}` is already registered")]
pub struct DuplicateTypeId(pub &'static str);

/// Maps `otp_type` ids to implementations.
///
/// An id with no implementation is *unsupported*, not invalid: [`OtpRegistry::get`]
/// returns `None` and the caller keeps the entry untouched (ADR-0004).
#[derive(Default)]
pub struct OtpRegistry {
    types: HashMap<&'static str, Box<dyn OtpType>>,
}

impl OtpRegistry {
    #[must_use]
    pub fn with_builtins() -> Self {
        let builtins: [Box<dyn OtpType>; 3] = [Box::new(Hotp), Box::new(Totp), Box::new(Steam)];
        Self {
            types: builtins.into_iter().map(|t| (t.id(), t)).collect(),
        }
    }

    /// # Errors
    /// If a type with the same id is already registered; built-ins cannot be shadowed.
    pub fn register(&mut self, otp_type: Box<dyn OtpType>) -> Result<(), DuplicateTypeId> {
        let id = otp_type.id();
        if self.types.contains_key(id) {
            return Err(DuplicateTypeId(id));
        }
        self.types.insert(id, otp_type);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn OtpType> {
        self.types.get(id).map(Box::as_ref)
    }

    /// Every registered OTP type, ordered by id.
    #[must_use]
    pub fn types(&self) -> Vec<&dyn OtpType> {
        let mut all: Vec<&dyn OtpType> = self.types.values().map(Box::as_ref).collect();
        all.sort_by_key(|otp_type| otp_type.id());
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otp::{Code, CodeTiming, OtpError, OtpInput, ParamError, ParamSpec, Params};
    use crate::secret::Secret;

    struct Fake;

    impl OtpType for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }

        fn timing(&self, _: &Params) -> Result<CodeTiming, ParamError> {
            Ok(CodeTiming::Counter(0))
        }

        fn schema(&self) -> &'static [ParamSpec] {
            &[]
        }

        fn generate(&self, _: &Secret, _: &Params, _: OtpInput) -> Result<Code, OtpError> {
            Ok(Code::new("000000".into()))
        }
    }

    #[test]
    fn builtins_are_found_by_id() {
        let registry = OtpRegistry::with_builtins();
        assert_eq!(registry.get("totp").map(OtpType::id), Some("totp"));
        assert_eq!(registry.get("hotp").map(OtpType::id), Some("hotp"));
        assert_eq!(registry.get("steam").map(OtpType::id), Some("steam"));
    }

    #[test]
    fn types_lists_everything_registered_in_id_order() {
        let mut registry = OtpRegistry::with_builtins();
        registry.register(Box::new(Fake)).unwrap();
        let ids: Vec<_> = registry.types().into_iter().map(OtpType::id).collect();
        assert_eq!(ids, ["fake", "hotp", "steam", "totp"]);
    }

    #[test]
    fn unknown_ids_are_unsupported_not_errors() {
        assert!(OtpRegistry::with_builtins().get("vendor-x").is_none());
    }

    #[test]
    fn new_types_can_be_registered() {
        let mut registry = OtpRegistry::with_builtins();
        registry.register(Box::new(Fake)).unwrap();
        assert_eq!(registry.get("fake").map(OtpType::id), Some("fake"));
    }

    #[test]
    fn builtins_cannot_be_shadowed() {
        let mut registry = OtpRegistry::with_builtins();
        assert_eq!(
            registry.register(Box::new(Totp)),
            Err(DuplicateTypeId("totp"))
        );
    }
}
