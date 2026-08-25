use serde_json::Value;

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum TemplateKind {
    Task,
    List,
    Folder,
}

impl TemplateKind {
    fn path_segment(self) -> &'static str {
        match self {
            TemplateKind::Task => "taskTemplate",
            TemplateKind::List => "list_template",
            TemplateKind::Folder => "folder_template",
        }
    }
}

/// `page` is required by ClickUp's template endpoints; defaults to `0` when not given.
pub async fn list(
    client: &CuClient,
    kind: TemplateKind,
    page: Option<u64>,
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/{}", kind.path_segment());
    let query = vec![("page".to_string(), page.unwrap_or(0).to_string())];
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segment_maps_each_kind() {
        assert_eq!(TemplateKind::Task.path_segment(), "taskTemplate");
        assert_eq!(TemplateKind::List.path_segment(), "list_template");
        assert_eq!(TemplateKind::Folder.path_segment(), "folder_template");
    }
}
