use std::{collections::HashMap, env::VarError};

use crate::ConfigureError;

/// Internal helper to get the `key` from the environment.
///
/// If `env` is provided, will use that instead of the process environment.
///
/// An empty value is treated the same as an unset variable, as required by the
/// OpenTelemetry specification and matching the Logfire Python SDK.
pub fn get_optional_env(
    key: &str,
    env: Option<&HashMap<String, String>>,
) -> Result<Option<String>, ConfigureError> {
    if let Some(env) = env {
        Ok(env.get(key).filter(|value| !value.is_empty()).cloned())
    } else {
        match std::env::var(key) {
            Ok(value) if value.is_empty() => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(VarError::NotPresent) => Ok(None),
            Err(VarError::NotUnicode(_)) => Err(ConfigureError::Other(
                format!("{key} is not valid UTF-8").into(),
            )),
        }
    }
}
