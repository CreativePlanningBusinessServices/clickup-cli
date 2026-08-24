#![allow(dead_code)]
use clickup_cli::client::CuClient;
use wiremock::MockServer;

pub const TEST_WORKSPACE: &str = "9001";

pub fn client_for(server: &MockServer) -> CuClient {
    CuClient::new(
        server.uri(),
        "TEST_TOKEN".into(),
        Some(TEST_WORKSPACE.into()),
    )
}
