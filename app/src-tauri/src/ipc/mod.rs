//! The narrow, typed surface between the UI and the core (ADR-0012).

pub mod commands;
pub mod dto;
pub mod error;
#[cfg(test)]
mod no_secrets;
#[cfg(test)]
mod registration;
pub mod service;
