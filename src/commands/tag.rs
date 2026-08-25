use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{encode_path_segment, pass_through};
use crate::error::CliError;

pub async fn list(client: &CuClient, space_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/tag");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `tag create` sends: `{"tag": {"name": ..., "tag_fg": ..., "tag_bg": ...}}`,
/// omitting the color keys when not given.
pub fn build_create_body(name: &str, fg: Option<&str>, bg: Option<&str>) -> Value {
    let mut tag = json!({"name": name});
    if let Some(fg) = fg {
        tag["tag_fg"] = json!(fg);
    }
    if let Some(bg) = bg {
        tag["tag_bg"] = json!(bg);
    }
    json!({"tag": tag})
}

pub async fn create(
    client: &CuClient,
    space_id: &str,
    name: &str,
    fg: Option<&str>,
    bg: Option<&str>,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/tag");
    let body = build_create_body(name, fg, bg);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(
    client: &CuClient,
    space_id: &str,
    tag_name: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/tag/{}", encode_tag_name(tag_name));
    let body = json!({"tag": data});
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, space_id: &str, tag_name: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/tag/{}", encode_tag_name(tag_name));
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "name": tag_name}))
}

/// Percent-encodes a tag name for use as a URL path segment — tag names can contain spaces
/// (e.g. `"high priority"` → `high%20priority`) or other reserved characters (e.g. a `/`,
/// which would otherwise split the path into extra segments), which aren't valid raw in a path
/// segment. Delegates to the shared `commands::encode_path_segment` helper.
fn encode_tag_name(tag_name: &str) -> String {
    encode_path_segment(tag_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_create_body_nests_name_and_colors() {
        let body = build_create_body("urgent", Some("#ffffff"), Some("#ff0000"));
        assert_eq!(
            body,
            json!({"tag": {"name": "urgent", "tag_fg": "#ffffff", "tag_bg": "#ff0000"}})
        );
    }

    #[test]
    fn build_create_body_omits_colors_when_absent() {
        let body = build_create_body("urgent", None, None);
        assert_eq!(body, json!({"tag": {"name": "urgent"}}));
    }

    #[test]
    fn encode_tag_name_percent_encodes_spaces_and_reserved_characters() {
        assert_eq!(encode_tag_name("high priority"), "high%20priority");
        assert_eq!(encode_tag_name("urgent"), "urgent");
        // A slash in a tag name must not be allowed to split the path into extra segments.
        assert_eq!(encode_tag_name("a/b"), "a%2Fb");
    }
}
