mod common;

use clickup_cli::commands::chat;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn channels_all_follows_cursor_over_data_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "chan1"}],
            "next_cursor": "c2",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels"))
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "chan2"}],
            "next_cursor": null,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = chat::channels(&common::client_for(&server), &[], true)
        .await
        .unwrap();

    assert_eq!(result["count"], 2);
    assert_eq!(result["hasMore"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn channels_without_all_fetches_a_single_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [{"id": "chan1"}]})))
        .expect(1)
        .mount(&server)
        .await;

    let result = chat::channels(&common::client_for(&server), &[], false)
        .await
        .unwrap();
    assert_eq!(result["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn channel_create_dm_sends_user_ids() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/channels/direct_message"))
        .and(body_json(json!({"user_ids": ["75413867", "12345"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "dm1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = chat::channel_create_dm(
        &common::client_for(&server),
        &["75413867".to_string(), "12345".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(result["id"], "dm1");
}

#[tokio::test]
async fn channel_create_merges_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/channels"))
        .and(body_json(json!({"name": "eng-updates"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "chan1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = chat::build_channel_create_body(json!({}), "eng-updates");
    let result = chat::channel_create(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["id"], "chan1");
}

#[tokio::test]
async fn channel_create_location_merges_location() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/channels/location"))
        .and(body_json(json!({"location": {"folder": {"id": "457"}}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "chan1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body =
        chat::build_channel_create_location_body(json!({}), json!({"folder": {"id": "457"}}));
    let result = chat::channel_create_location(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["id"], "chan1");
}

#[tokio::test]
async fn channel_get_update_delete_hit_channel_id_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "chan1"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1"))
        .and(body_json(json!({"name": "renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "chan1"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    chat::channel_get(&client, "chan1").await.unwrap();
    chat::channel_update(&client, "chan1", json!({"name": "renamed"}))
        .await
        .unwrap();
    let delete_result = chat::channel_delete(&client, "chan1").await.unwrap();
    assert_eq!(
        delete_result,
        json!({"deleted": true, "channel_id": "chan1"})
    );
}

#[tokio::test]
async fn channel_members_and_followers_hit_their_paths() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1/members"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v3/workspaces/9001/chat/channels/chan1/followers",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    chat::channel_members(&client, "chan1", None, false)
        .await
        .unwrap();
    chat::channel_followers(&client, "chan1", None, false)
        .await
        .unwrap();
}

#[tokio::test]
async fn messages_hits_channel_messages_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [{"id": "msg1"}]})))
        .expect(1)
        .mount(&server)
        .await;

    let result = chat::messages(&common::client_for(&server), "chan1", None, false)
        .await
        .unwrap();
    assert_eq!(result["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn send_defaults_type_message_and_merges_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1/messages"))
        .and(body_json(
            json!({"type": "message", "content": "build is green"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "msg1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = chat::build_send_body(json!({}), "build is green", None);
    let result = chat::send(&common::client_for(&server), "chan1", body)
        .await
        .unwrap();
    assert_eq!(result["id"], "msg1");
}

#[tokio::test]
async fn send_type_flag_overrides_default() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/channels/chan1/messages"))
        .and(body_json(json!({"type": "post", "content": "posting"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "msg1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = chat::build_send_body(json!({}), "posting", Some("post"));
    chat::send(&common::client_for(&server), "chan1", body)
        .await
        .unwrap();
}

#[tokio::test]
async fn message_update_and_delete_hit_message_id_path() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v3/workspaces/9001/chat/messages/msg1"))
        .and(body_json(json!({"content": "edited"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "msg1"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v3/workspaces/9001/chat/messages/msg1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    chat::message_update(&client, "msg1", json!({"content": "edited"}))
        .await
        .unwrap();
    let delete_result = chat::message_delete(&client, "msg1").await.unwrap();
    assert_eq!(
        delete_result,
        json!({"deleted": true, "message_id": "msg1"})
    );
}

#[tokio::test]
async fn replies_and_reply_hit_replies_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/messages/msg1/replies"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/messages/msg1/replies"))
        .and(body_json(json!({"content": "ack"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "msg2"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    chat::replies(&client, "msg1", None, false).await.unwrap();
    let body = chat::build_reply_body(json!({}), "ack");
    let result = chat::reply(&client, "msg1", body).await.unwrap();
    assert_eq!(result["id"], "msg2");
}

#[tokio::test]
async fn react_and_unreact_hit_reaction_paths() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/workspaces/9001/chat/messages/msg1/reactions"))
        .and(body_json(json!({"reaction": "heart"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"reaction": "heart"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(
            "/api/v3/workspaces/9001/chat/messages/msg1/reactions/heart",
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let react_result = chat::react(&client, "msg1", "heart").await.unwrap();
    assert_eq!(react_result["reaction"], "heart");

    let unreact_result = chat::unreact(&client, "msg1", "heart").await.unwrap();
    assert_eq!(
        unreact_result,
        json!({"deleted": true, "message_id": "msg1", "reaction": "heart"})
    );
}

/// `reaction` is free text interpolated into the URL path (not a query param, which reqwest
/// already encodes), so a reaction name like `"+1"` must be percent-encoded before it's sent —
/// otherwise a raw `+` could be decoded as a space by the server, or otherwise malform the path.
#[tokio::test]
async fn unreact_percent_encodes_reaction_with_special_characters() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(
            "/api/v3/workspaces/9001/chat/messages/msg1/reactions/%2B1",
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let unreact_result = chat::unreact(&client, "msg1", "+1").await.unwrap();
    assert_eq!(
        unreact_result,
        json!({"deleted": true, "message_id": "msg1", "reaction": "+1"})
    );
}

#[tokio::test]
async fn reactions_hits_reactions_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/messages/msg1/reactions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .expect(1)
        .mount(&server)
        .await;

    chat::reactions(&common::client_for(&server), "msg1", None, false)
        .await
        .unwrap();
}

#[tokio::test]
async fn tagged_users_hits_tagged_users_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v3/workspaces/9001/chat/messages/msg1/tagged_users",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"users": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = chat::tagged_users(&common::client_for(&server), "msg1")
        .await
        .unwrap();
    assert_eq!(result["users"], json!([]));
}

#[tokio::test]
async fn merge_cursor_pages_strips_a_seeded_cursor_when_fetching_all() {
    use clickup_cli::commands::merge_cursor_pages;

    let server = MockServer::start().await;
    // Page 1 must NOT carry the stale caller-seeded cursor.
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "chan1"}],
            "next_cursor": "c2"
        })))
        .expect(1)
        .mount(&server)
        .await;
    // Page 2 carries exactly the loop's own cursor.
    Mock::given(method("GET"))
        .and(path("/api/v3/workspaces/9001/chat/channels"))
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "chan2"}],
            "next_cursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let seeded = vec![("cursor".to_string(), "stale".to_string())];
    let result = merge_cursor_pages(
        &common::client_for(&server),
        "/api/v3/workspaces/9001/chat/channels",
        &seeded,
        "data",
        true,
    )
    .await
    .unwrap();

    assert_eq!(result["count"], 2);
    assert_eq!(result["hasMore"], false);
    // Prove no request ever carried two cursor params or the stale value.
    for request in server.received_requests().await.unwrap() {
        let cursor_pairs: Vec<_> = request
            .url
            .query_pairs()
            .filter(|(key, _)| key == "cursor")
            .collect();
        assert!(cursor_pairs.len() <= 1);
        assert!(cursor_pairs.iter().all(|(_, value)| value != "stale"));
    }
}
