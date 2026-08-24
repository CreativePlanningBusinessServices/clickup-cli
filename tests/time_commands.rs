mod common;

use clickup_cli::commands::time::{self, TimeListArgs, build_entry_body};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn base_args() -> TimeListArgs {
    TimeListArgs {
        start_date: None,
        end_date: None,
        assignee: None,
        task_id: None,
        list_id: None,
        space_id: None,
        folder_id: None,
        billable: None,
        include_task_tags: false,
        include_location_names: false,
    }
}

#[tokio::test]
async fn list_passes_date_window_and_assignee() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/time_entries"))
        .and(query_param("start_date", "1735689600000"))
        .and(query_param("end_date", "1738368000000"))
        .and(query_param("assignee", "75413867"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    let mut args = base_args();
    args.start_date = Some(1735689600000);
    args.end_date = Some(1738368000000);
    args.assignee = Some("75413867".to_string());

    let result = time::list(&common::client_for(&server), &args)
        .await
        .unwrap();
    assert_eq!(result["data"], json!([]));
}

#[tokio::test]
async fn get_hits_time_entry_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/time_entries/4190"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "4190"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = time::get(&common::client_for(&server), "4190")
        .await
        .unwrap();
    assert_eq!(result["id"], "4190");
}

/// Drives `create` through `build_entry_body` — the same builder `dispatch_time`'s `Create`
/// arm calls in `cli.rs` — so this exercises the `--task` → `tid` merge, not just passthrough.
#[tokio::test]
async fn create_merges_task_flag_as_tid() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/time_entries"))
        .and(body_json(json!({
            "start": 1_735_689_600_000_i64,
            "duration": 3_600_000,
            "tid": "868k5a5wa"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "4190"})))
        .expect(1)
        .mount(&server)
        .await;

    let data = json!({"start": 1_735_689_600_000_i64, "duration": 3_600_000});
    let body = build_entry_body(data, Some("868k5a5wa"), None, None);
    let result = time::create(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["id"], "4190");
}

#[tokio::test]
async fn update_and_delete_hit_time_entry_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/team/9001/time_entries/4190"))
        .and(body_json(json!({"description": "pairing"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "4190"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/team/9001/time_entries/4190"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let update_result = time::update(&client, "4190", json!({"description": "pairing"}))
        .await
        .unwrap();
    assert_eq!(update_result["id"], "4190");

    let delete_result = time::delete(&client, "4190").await.unwrap();
    assert_eq!(delete_result, json!({"deleted": true, "id": "4190"}));
}

#[tokio::test]
async fn history_hits_history_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/time_entries/4190/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"history": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = time::history(&common::client_for(&server), "4190")
        .await
        .unwrap();
    assert_eq!(result["history"], json!([]));
}

#[tokio::test]
async fn current_passes_assignee_when_given() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/time_entries/current"))
        .and(query_param("assignee", "75413867"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": null})))
        .expect(1)
        .mount(&server)
        .await;

    let result = time::current(&common::client_for(&server), Some("75413867"))
        .await
        .unwrap();
    assert_eq!(result["data"], json!(null));
}

#[tokio::test]
async fn start_posts_to_start_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/time_entries/start"))
        .and(body_json(
            json!({"tid": "868k5a5wa", "description": "pairing"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "4191"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = build_entry_body(json!({}), Some("868k5a5wa"), Some("pairing"), None);
    let result = time::start(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["id"], "4191");
}

#[tokio::test]
async fn stop_posts_to_stop_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/time_entries/stop"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "4191"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = time::stop(&common::client_for(&server)).await.unwrap();
    assert_eq!(result["id"], "4191");
}

#[tokio::test]
async fn tags_hits_tags_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/time_entries/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = time::tags(&common::client_for(&server)).await.unwrap();
    assert_eq!(result["data"], json!([]));
}

#[tokio::test]
async fn tag_add_sends_ids_and_tags_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/time_entries/tags"))
        .and(body_json(json!({
            "time_entry_ids": ["123", "456"],
            "tags": ["billing"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    let entry_ids = vec!["123".to_string(), "456".to_string()];
    let tags = vec!["billing".to_string()];
    let result = time::tag_add(&common::client_for(&server), &entry_ids, &tags)
        .await
        .unwrap();
    assert_eq!(result["data"], json!([]));
}

/// ClickUp's remove-tags-from-time-entries endpoint takes the ids/tags in a JSON body sent
/// along with the DELETE request rather than query params; confirm the body actually rode
/// along on the DELETE by inspecting the request the mock server received.
#[tokio::test]
async fn tag_remove_sends_delete_with_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/team/9001/time_entries/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    let entry_ids = vec!["123".to_string()];
    let tags = vec!["billing".to_string()];
    time::tag_remove(&common::client_for(&server), &entry_ids, &tags)
        .await
        .unwrap();

    let received_requests = server.received_requests().await.unwrap();
    assert_eq!(received_requests.len(), 1);
    let request = &received_requests[0];
    let body: serde_json::Value = request.body_json().unwrap();
    assert_eq!(
        body,
        json!({"time_entry_ids": ["123"], "tags": ["billing"]})
    );
}

#[tokio::test]
async fn tag_rename_puts_name_and_colors() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/team/9001/time_entries/tags"))
        .and(body_json(json!({
            "name": "billing",
            "new_name": "billable-work",
            "tag_fg": "#ffffff",
            "tag_bg": "#00ff00"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = time::tag_rename(
        &common::client_for(&server),
        "billing",
        "billable-work",
        Some("#ffffff"),
        Some("#00ff00"),
    )
    .await
    .unwrap();
    assert_eq!(result["data"], json!([]));
}
