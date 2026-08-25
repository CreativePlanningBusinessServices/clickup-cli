use std::path::Path;

use serde_json::{Value, json};

use crate::config::Config;
use crate::error::CliError;

const VALID_KEYS: [&str; 1] = ["default_account"];

pub fn get(config_path: &Path, key: Option<&str>) -> Result<Value, CliError> {
    let config = Config::load(config_path)?;
    match key {
        None => Ok(serde_json::to_value(&config).expect("config serializable")),
        Some("default_account") => Ok(json!({"default_account": config.default_account})),
        Some(unknown_key) => Err(unknown_key_error(unknown_key)),
    }
}

pub fn set(config_path: &Path, key: &str, value: &str) -> Result<Value, CliError> {
    let mut config = Config::load(config_path)?;
    let result = match key {
        "default_account" => {
            if !config.accounts.contains_key(value) {
                return Err(CliError::Usage(format!(
                    "unknown account alias '{value}' — run `clickup-cli account list`"
                )));
            }
            config.default_account = Some(value.to_string());
            json!({"default_account": value})
        }
        unknown_key => return Err(unknown_key_error(unknown_key)),
    };
    config.save(config_path)?;
    Ok(result)
}

fn unknown_key_error(key: &str) -> CliError {
    CliError::Usage(format!(
        "unknown config key '{key}' — known keys: {}",
        VALID_KEYS.join(", ")
    ))
}
