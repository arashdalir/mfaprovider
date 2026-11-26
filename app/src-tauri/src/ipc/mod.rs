//! The narrow, typed surface between the UI and the core (ADR-0012).

pub mod biometric_service;
pub mod commands;
pub mod device_settings;
pub mod dto;
pub mod error;
#[cfg(test)]
mod no_secrets;
mod picker_excuse;
#[cfg(test)]
mod registration;
pub mod service;
