//! OTP types behind one interface (ADR-0007). TOTP and HOTP are built in.

mod algorithm;
mod hotp;
mod otp_type;
mod params;
mod registry;
mod totp;

pub use algorithm::Algorithm;
pub use hotp::Hotp;
pub use otp_type::{Code, OtpError, OtpInput, OtpType};
pub use params::{resolve, ParamError, ParamKind, ParamSpec, Params, Resolved};
pub use registry::{DuplicateTypeId, OtpRegistry};
pub use totp::Totp;
