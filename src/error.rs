use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{message}")]
    Api {
        status: u16,
        message: String,
        ecode: Option<String>,
    },
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Auth(String),
    #[error("{0}")]
    Network(String),
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Api { .. } => 1,
            CliError::Usage(_) => 2,
            CliError::Auth(_) => 3,
            CliError::Network(_) => 4,
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            CliError::Api {
                status,
                message,
                ecode,
            } => {
                let mut envelope = json!({"kind": "api", "status": status, "message": message});
                if let Some(code) = ecode {
                    envelope["ecode"] = json!(code);
                }
                envelope
            }
            CliError::Usage(message) => json!({"kind": "usage", "message": message}),
            CliError::Auth(message) => json!({"kind": "auth", "message": message}),
            CliError::Network(message) => json!({"kind": "network", "message": message}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_error_kinds() {
        assert_eq!(
            CliError::Api {
                status: 400,
                message: "bad".into(),
                ecode: None
            }
            .exit_code(),
            1
        );
        assert_eq!(CliError::Usage("no account".into()).exit_code(), 2);
        assert_eq!(CliError::Auth("token rejected".into()).exit_code(), 3);
        assert_eq!(CliError::Network("timeout".into()).exit_code(), 4);
    }

    #[test]
    fn api_error_serializes_full_envelope_with_ecode() {
        let error = CliError::Api {
            status: 401,
            message: "Token invalid".into(),
            ecode: Some("OAUTH_025".into()),
        };
        assert_eq!(
            error.to_json(),
            json!({"kind": "api", "status": 401, "message": "Token invalid", "ecode": "OAUTH_025"})
        );
    }

    #[test]
    fn api_error_omits_ecode_when_absent() {
        let error = CliError::Api {
            status: 404,
            message: "not found".into(),
            ecode: None,
        };
        assert_eq!(
            error.to_json(),
            json!({"kind": "api", "status": 404, "message": "not found"})
        );
    }

    #[test]
    fn auth_error_serializes_without_status() {
        assert_eq!(
            CliError::Auth("no token stored".into()).to_json(),
            json!({"kind": "auth", "message": "no token stored"})
        );
    }
}
