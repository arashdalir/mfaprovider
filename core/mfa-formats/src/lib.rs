//! Codecs between the outside world and the core's entry definitions (ADR-0004, ADR-0005).
//! No I/O: callers pass text or bytes in and get entries or errors out.

pub mod archive;
pub mod import;
pub mod merge;
pub mod migration;
pub mod otpauth;
pub mod qr;
pub mod qr_encode;
