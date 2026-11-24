//! OTP types behind one interface (ADR-0007). TOTP, HOTP and Steam Guard are built in.

mod algorithm;
mod hotp;
mod live;
mod otp_type;
mod params;
mod registry;
mod steam;
mod totp;

pub use algorithm::Algorithm;
pub use hotp::Hotp;
pub use live::{live_code, LiveCode, Validity};
pub use otp_type::{Code, CodeTiming, OtpError, OtpInput, OtpType};
pub use params::{resolve, ParamError, ParamKind, ParamSpec, Params, Resolved};
pub use registry::{DuplicateTypeId, OtpRegistry};
pub use steam::Steam;
pub use totp::Totp;
