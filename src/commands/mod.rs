pub mod account;

use std::io::Read;

use serde_json::Value;

use crate::error::CliError;

/// Reads a `--data`-style argument: inline JSON, `@file` to read a file, or `-` for stdin.
pub fn read_data_arg(data: &str) -> Result<Value, CliError> {
    let text = if data == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|io_error| CliError::Usage(format!("cannot read stdin: {io_error}")))?;
        buffer
    } else if let Some(file_path) = data.strip_prefix('@') {
        std::fs::read_to_string(file_path)
            .map_err(|io_error| CliError::Usage(format!("cannot read {file_path}: {io_error}")))?
    } else {
        data.to_string()
    };
    serde_json::from_str(&text)
        .map_err(|parse_error| CliError::Usage(format!("data is not valid JSON: {parse_error}")))
}

/// Parses repeated `key=value` flags.
pub fn parse_key_value_pairs(pairs: &[String]) -> Result<Vec<(String, String)>, CliError> {
    pairs
        .iter()
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| CliError::Usage(format!("expected key=value, got '{pair}'")))?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn reads_inline_json() {
        let parsed = read_data_arg(r#"{"name": "Acme"}"#).unwrap();
        assert_eq!(parsed, serde_json::json!({"name": "Acme"}));
    }

    #[test]
    fn reads_json_from_file_argument() {
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        write!(temp_file, r#"{{"name": "Acme"}}"#).unwrap();
        let file_arg = format!("@{}", temp_file.path().display());

        let parsed = read_data_arg(&file_arg).unwrap();
        assert_eq!(parsed, serde_json::json!({"name": "Acme"}));
    }

    #[test]
    fn rejects_invalid_json() {
        let parse_result = read_data_arg("not json");
        assert!(matches!(parse_result, Err(CliError::Usage(_))));
    }

    #[test]
    fn rejects_missing_file() {
        let parse_result = read_data_arg("@/nonexistent");
        assert!(matches!(parse_result, Err(CliError::Usage(_))));
    }

    #[test]
    fn parses_key_value_pairs() {
        let pairs =
            parse_key_value_pairs(&["status=open".to_string(), "priority=1".to_string()]).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("status".to_string(), "open".to_string()),
                ("priority".to_string(), "1".to_string())
            ]
        );
    }

    #[test]
    fn rejects_pair_without_equals() {
        let result = parse_key_value_pairs(&["status".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }
}
