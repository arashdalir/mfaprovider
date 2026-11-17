use std::fmt;

use thiserror::Error;

use super::params::{resolve, ParamError, ParamSpec, Params};
use crate::secret::Secret;

/// What a code is derived from: seconds since the Unix epoch, or an HOTP counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OtpInput {
    Time(u64),
    Counter(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code(String);

impl Code {
    pub(super) fn new(text: String) -> Self {
        Self(text)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OtpError {
    #[error(transparent)]
    Params(#[from] ParamError),
    #[error("the secret is empty")]
    EmptySecret,
    #[error("this OTP type needs {expected} as input")]
    WrongInput { expected: &'static str },
}

pub trait OtpType: Send + Sync {
    fn id(&self) -> &'static str;

    fn schema(&self) -> &'static [ParamSpec];

    /// # Errors
    /// On invalid parameters, an empty secret, or an input kind this type cannot use.
    fn generate(&self, secret: &Secret, params: &Params, input: OtpInput)
        -> Result<Code, OtpError>;

    /// # Errors
    /// If `params` do not satisfy [`OtpType::schema`].
    fn validate_params(&self, params: &Params) -> Result<(), ParamError> {
        resolve(self.schema(), params).map(|_| ())
    }
}
