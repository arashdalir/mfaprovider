use std::collections::BTreeMap;

use thiserror::Error;

/// Type-specific entry parameters as stored in the vault (ADR-0004).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Params(BTreeMap<String, String>);

impl Params {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.0.insert(name.to_owned(), value.to_owned());
        self
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// Name/value pairs in name order, so serialisation is deterministic.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    Integer { min: u64, max: u64 },
    Choice(&'static [&'static str]),
}

/// One declared parameter of an OTP type; also what a UI form is generated from.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub name: &'static str,
    pub kind: ParamKind,
    pub default: &'static str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParamError {
    #[error("unknown parameter `{0}`")]
    Unknown(String),
    #[error("parameter `{name}` must be an integer between {min} and {max}")]
    InvalidInteger {
        name: &'static str,
        min: u64,
        max: u64,
    },
    #[error("parameter `{name}` must be one of {choices:?}")]
    InvalidChoice {
        name: &'static str,
        choices: &'static [&'static str],
    },
}

#[derive(Clone, Copy, Debug)]
enum Value {
    Integer(u64),
    Choice(&'static str),
}

/// Parameters validated against a schema, with defaults applied.
#[derive(Debug)]
pub struct Resolved(BTreeMap<&'static str, Value>);

impl Resolved {
    /// # Panics
    /// If `name` is not an integer parameter of the schema this was resolved against.
    #[must_use]
    pub fn integer(&self, name: &str) -> u64 {
        match self.0.get(name) {
            Some(Value::Integer(value)) => *value,
            _ => panic!("`{name}` is not an integer parameter of the schema"),
        }
    }

    /// # Panics
    /// If `name` is not a choice parameter of the schema this was resolved against.
    #[must_use]
    pub fn choice(&self, name: &str) -> &'static str {
        match self.0.get(name) {
            Some(Value::Choice(value)) => value,
            _ => panic!("`{name}` is not a choice parameter of the schema"),
        }
    }
}

/// # Errors
/// On an unknown parameter name or a value outside the declared range/choices.
pub fn resolve(schema: &[ParamSpec], params: &Params) -> Result<Resolved, ParamError> {
    if let Some(unknown) = params.names().find(|n| schema.iter().all(|s| s.name != *n)) {
        return Err(ParamError::Unknown(unknown.to_owned()));
    }
    schema
        .iter()
        .map(|spec| {
            Ok((
                spec.name,
                parse(spec, params.get(spec.name).unwrap_or(spec.default))?,
            ))
        })
        .collect::<Result<_, _>>()
        .map(Resolved)
}

fn parse(spec: &ParamSpec, raw: &str) -> Result<Value, ParamError> {
    match spec.kind {
        ParamKind::Integer { min, max } => raw
            .parse::<u64>()
            .ok()
            .filter(|value| (min..=max).contains(value))
            .map(Value::Integer)
            .ok_or(ParamError::InvalidInteger {
                name: spec.name,
                min,
                max,
            }),
        ParamKind::Choice(choices) => choices
            .iter()
            .find(|choice| choice.eq_ignore_ascii_case(raw))
            .map(|choice| Value::Choice(choice))
            .ok_or(ParamError::InvalidChoice {
                name: spec.name,
                choices,
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &[ParamSpec] = &[
        ParamSpec {
            name: "digits",
            kind: ParamKind::Integer { min: 6, max: 8 },
            default: "6",
        },
        ParamSpec {
            name: "algorithm",
            kind: ParamKind::Choice(&["SHA1", "SHA256"]),
            default: "SHA1",
        },
    ];

    #[test]
    fn defaults_apply_when_parameters_are_absent() {
        let resolved = resolve(SCHEMA, &Params::new()).unwrap();
        assert_eq!(resolved.integer("digits"), 6);
        assert_eq!(resolved.choice("algorithm"), "SHA1");
    }

    #[test]
    fn explicit_values_override_defaults_and_choices_ignore_case() {
        let params = Params::new()
            .with("digits", "8")
            .with("algorithm", "sha256");
        let resolved = resolve(SCHEMA, &params).unwrap();
        assert_eq!(resolved.integer("digits"), 8);
        assert_eq!(resolved.choice("algorithm"), "SHA256");
    }

    #[test]
    fn rejects_unknown_parameters() {
        let params = Params::new().with("color", "red");
        assert_eq!(
            resolve(SCHEMA, &params).unwrap_err(),
            ParamError::Unknown("color".into())
        );
    }

    #[test]
    fn rejects_out_of_range_and_malformed_integers() {
        for bad in ["5", "9", "six", "-1", ""] {
            let params = Params::new().with("digits", bad);
            assert!(
                matches!(
                    resolve(SCHEMA, &params),
                    Err(ParamError::InvalidInteger { .. })
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn rejects_unlisted_choices() {
        let params = Params::new().with("algorithm", "MD5");
        assert!(matches!(
            resolve(SCHEMA, &params),
            Err(ParamError::InvalidChoice { .. })
        ));
    }
}
