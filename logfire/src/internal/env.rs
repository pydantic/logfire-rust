use std::{collections::HashMap, env::VarError};

use crate::ConfigureError;

/// Internal helper to get the `key` from the environment.
///
/// If `env` is provided, will use that instead of the process environment.
pub fn get_optional_env(
    key: &str,
    env: Option<&HashMap<String, String>>,
) -> Result<Option<String>, ConfigureError> {
    if let Some(env) = env {
        Ok(env.get(key).cloned())
    } else {
        match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(VarError::NotPresent) => Ok(None),
            Err(VarError::NotUnicode(_)) => Err(ConfigureError::Other(
                format!("{key} is not valid UTF-8").into(),
            )),
        }
    }
}
