mod common;

use clickup_cli::commands::doc;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn search_all_follows_next_cursor_over_docs_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "docs": [{"id": "doc1"}],
            "next_cursor": "c2",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs"))
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "docs": [{"id": "doc2"}],
            "next_cursor": null,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = doc::search(&common::client_for(&server), &[], true)
        .await
        .unwrap();

    assert_eq!(result["count"], 2);
    assert_eq!(result["hasMore"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn search_without_all_fetches_a_single_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs"))
        .and(query_param("parent_id", "790"))
        .and(query_param("parent_type", "4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"docs": [{"id": "doc1"}]})))
        .expect(1)
        .mount(&server)
        .await;

    let query = doc::build_search_query(Some("790"), Some(4), None, None, None, None, None);
    let result = doc::search(&common::client_for(&server), &query, false)
        .await
        .unwrap();
    assert_eq!(result["docs"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn create_builds_parent_object_from_flags() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/docs"))
        .and(body_json(
            json!({"name": "Runbook", "parent": {"id": "790", "type": 4}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "doc1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = doc::build_create_body(json!({}), "Runbook", "790", 4);
    let result = doc::create(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["id"], "doc1");
}

#[tokio::test]
async fn get_hits_doc_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs/doc1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "doc1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = doc::get(&common::client_for(&server), "doc1")
        .await
        .unwrap();
    assert_eq!(result["id"], "doc1");
}

#[tokio::test]
async fn page_listing_passes_depth_as_max_page_depth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs/doc1/page_listing"))
        .and(query_param("max_page_depth", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pages": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = doc::page_listing(&common::client_for(&server), "doc1", Some(2))
        .await
        .unwrap();
    assert_eq!(result["pages"], json!([]));
}

#[tokio::test]
async fn pages_passes_depth_and_format() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs/doc1/pages"))
        .and(query_param("max_page_depth", "1"))
        .and(query_param("content_format", "text/plain"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pages": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = doc::pages(
        &common::client_for(&server),
        "doc1",
        Some(1),
        Some("text/plain"),
    )
    .await
    .unwrap();
    assert_eq!(result["pages"], json!([]));
}

#[tokio::test]
async fn page_get_passes_content_format() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/docs/doc1/pages/page1"))
        .and(query_param("content_format", "text/md"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "page1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = doc::page_get(
        &common::client_for(&server),
        "doc1",
        "page1",
        Some("text/md"),
    )
    .await
    .unwrap();
    assert_eq!(result["id"], "page1");
}

#[tokio::test]
async fn page_create_sends_string_content_untouched() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/docs/doc1/pages"))
        .and(body_json(
            json!({"name": "Setup", "content": "# Setup\n\nsteps"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "page1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = doc::build_page_create_body(
        json!({}),
        None,
        Some("Setup"),
        None,
        Some("# Setup\n\nsteps".to_string()),
        None,
    );
    let result = doc::page_create(&common::client_for(&server), "doc1", body)
        .await
        .unwrap();
    assert_eq!(result["id"], "page1");
}

#[tokio::test]
async fn page_update_sends_mode_and_string_content() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v3/workspaces/9001/docs/doc1/pages/page1"))
        .and(body_json(json!({
            "content": "more notes",
            "content_edit_mode": "append",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "page1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = doc::build_page_update_body(
        json!({}),
        None,
        None,
        Some("more notes".to_string()),
        Some("append"),
        None,
    );
    let result = doc::page_update(&common::client_for(&server), "doc1", "page1", body)
        .await
        .unwrap();
    assert_eq!(result["id"], "page1");
}
