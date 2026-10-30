//! Tests for discovery and the framed client, run against the fake bridge
//! listening on a real Unix socket in a temporary endpoint directory.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use gpui_mcp_protocol::{AppId, BridgeError, BridgeResult, ErrorCode, RequestId, WireResponse};
use tempfile::TempDir;

use super::client::{CallError, IO_TIMEOUT};
use super::descriptor::{InvalidTargetId, TargetId};
use super::discover::discover;
use super::fake::{Behavior, DEAD_PID, FakeBridge, pong, start_in};
use super::session::BridgeSession;

/// How long a test client waits for the bridge before giving up. Short tests
/// need a short timeout; production uses the 10 s default.
const TEST_TIMEOUT: Duration = Duration::from_millis(250);

fn descriptor_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_json(dir: &Path, name: &str, value: &serde_json::Value) {
    fs::write(dir.join(name), serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn target_id_round_trips() {
    let id = TargetId::from_parts(
        &AppId::new("reqflux").unwrap(),
        gpui_mcp_protocol::InstanceId::new(0xff).unwrap(),
    );
    assert_eq!(id.as_str(), "reqflux-00000000000000ff");
    assert_eq!(id.to_string(), "reqflux-00000000000000ff");
    assert_eq!(TargetId::from_str("reqflux-00000000000000ff").unwrap(), id);
    for rejected in [
        "reqflux",
        "reqflux-0",
        "reqflux-00000000000000000",
        "reqflux-0000000000000000",
        "reqflux-00000000000000zz",
        "-00000000000000ff",
        "not an app-00000000000000ff",
    ] {
        assert_eq!(
            TargetId::from_str(rejected),
            Err(InvalidTargetId),
            "{rejected}"
        );
    }
}

#[tokio::test]
async fn discovery_accepts_live_targets_sorted() {
    let dir = TempDir::new().unwrap();
    let _second = start_in(
        dir.path(),
        "reqflux",
        0x02,
        Behavior::Respond(Arc::new(pong)),
    );
    let first = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );

    let targets = discover(dir.path(), None).await.unwrap();
    let ids: Vec<_> = targets
        .iter()
        .map(|target| target.target_id().as_str().to_owned())
        .collect();
    assert_eq!(
        ids,
        ["reqflux-0000000000000001", "reqflux-0000000000000002"]
    );

    let other = AppId::new("other").unwrap();
    assert!(discover(dir.path(), Some(&other)).await.unwrap().is_empty());
    drop(first);
}

#[tokio::test]
async fn discovery_skips_unusable_descriptors() {
    let dir = TempDir::new().unwrap();
    let good = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );
    let good_json = descriptor_json(&good.descriptor_path);

    let mut value = good_json.clone();
    value["protocol_version"] = serde_json::json!(1);
    write_json(dir.path(), "wrong-protocol.json", &value);

    let mut value = good_json.clone();
    value["token"] = serde_json::json!("zz".repeat(32));
    write_json(dir.path(), "bad-token.json", &value);

    let mut value = good_json.clone();
    value["pid"] = serde_json::json!(DEAD_PID);
    write_json(dir.path(), "dead-pid.json", &value);

    fs::write(dir.path().join("plain.sock"), b"not a socket").unwrap();
    let mut value = good_json.clone();
    value["endpoint"] = serde_json::json!({
        "kind": "filesystem",
        "path": dir.path().join("plain.sock"),
    });
    write_json(dir.path(), "plain-endpoint.json", &value);

    let elsewhere = TempDir::new().unwrap();
    let mut value = good_json.clone();
    value["endpoint"] = serde_json::json!({
        "kind": "filesystem",
        "path": elsewhere.path().join("absent.sock"),
    });
    write_json(dir.path(), "outside-endpoint.json", &value);

    symlink("nowhere.json", dir.path().join("linked.json")).unwrap();

    let copied = dir.path().join("group-readable.json");
    fs::copy(&good.descriptor_path, &copied).unwrap();
    fs::set_permissions(&copied, fs::Permissions::from_mode(0o640)).unwrap();

    fs::write(dir.path().join("big.json"), vec![b'x'; 20 * 1024]).unwrap();

    let targets = discover(dir.path(), None).await.unwrap();
    assert_eq!(targets.len(), 1, "only the valid descriptor survives");
    assert_eq!(targets[0].target_id(), good.target_id);
}

#[tokio::test]
async fn discovery_is_capped_at_128_candidates() {
    let dir = TempDir::new().unwrap();
    let good = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );
    for index in 0..129 {
        // Invalid content, but each file occupies one candidate slot ahead of
        // the valid descriptor in name order.
        fs::write(dir.path().join(format!("filler-{index:03}.json")), b"{}").unwrap();
    }
    assert!(discover(dir.path(), None).await.unwrap().is_empty());
    drop(good);
}

#[tokio::test]
async fn ping_receives_a_pong() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Respond(Arc::new(pong)));
    let client = bridge.client(IO_TIMEOUT).await;
    let result = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap();
    assert!(matches!(result, BridgeResult::Pong { app_id, .. } if app_id.as_str() == "reqflux"));
}

#[tokio::test]
async fn a_mismatched_request_id_is_an_error() {
    let bridge = FakeBridge::new(
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(|request| WireResponse {
            protocol_version: gpui_mcp_protocol::PROTOCOL_VERSION,
            request_id: RequestId::new(request.request_id.get() + 1).unwrap(),
            result: Some(BridgeResult::Ack),
            error: None,
        })),
    );
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap_err();
    assert!(matches!(error, CallError::RequestIdMismatch), "{error}");
}

#[tokio::test]
async fn a_mismatched_protocol_version_is_an_error() {
    let bridge = FakeBridge::new(
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(|request| WireResponse {
            protocol_version: 1,
            request_id: request.request_id,
            result: Some(BridgeResult::Ack),
            error: None,
        })),
    );
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap_err();
    assert!(
        matches!(error, CallError::ProtocolMismatch(found, expected) if found == 1 && expected == gpui_mcp_protocol::PROTOCOL_VERSION),
        "{error}"
    );
}

#[tokio::test]
async fn a_bridge_error_maps_to_its_code_and_message() {
    let bridge = FakeBridge::new(
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(|request| {
            WireResponse::failure(
                request.request_id,
                BridgeError::new(ErrorCode::Unauthorized, "bad token"),
            )
        })),
    );
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap_err();
    let CallError::Bridge(code, message) = error else {
        panic!("expected a bridge error, got {error}");
    };
    assert_eq!(code, ErrorCode::Unauthorized);
    assert_eq!(message, "bad token");
}

#[tokio::test]
async fn a_server_that_closes_the_socket_is_an_error() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Close);
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap_err();
    assert!(matches!(error, CallError::ConnectionClosed), "{error}");
}

#[tokio::test]
async fn an_oversize_response_is_refused() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Oversize);
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap_err();
    assert!(matches!(error, CallError::ResponseSize), "{error}");
}

#[tokio::test]
async fn a_silent_server_times_out() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Silent);
    let client = bridge.client(TEST_TIMEOUT).await;
    let error = client
        .call(gpui_mcp_protocol::Operation::Ping)
        .await
        .unwrap_err();
    assert!(matches!(error, CallError::ResponseTimeout), "{error}");
}

// The session: which target the `ui_*` tools talk to (S3, S4).

fn session(dir: &Path) -> BridgeSession {
    BridgeSession::new(dir.to_path_buf(), AppId::new("reqflux").unwrap())
}

#[tokio::test]
async fn a_session_selects_the_only_target() {
    let dir = TempDir::new().unwrap();
    let bridge = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );
    let mut session = session(dir.path());

    let targets = session.list_targets().await.unwrap();
    assert_eq!(targets.len(), 1);
    assert!(targets[0].selected, "the only target is selected");
    assert_eq!(
        session.client().await.unwrap().target_id(),
        bridge.target_id
    );
}

#[tokio::test]
async fn a_session_without_a_target_names_the_app() {
    let dir = TempDir::new().unwrap();
    let error = session(dir.path()).client().await.unwrap_err();
    assert!(error.to_string().contains("reqflux"), "{error}");
}

#[tokio::test]
async fn a_session_asks_which_target_when_several_are_running() {
    let dir = TempDir::new().unwrap();
    let _first = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );
    let second = start_in(
        dir.path(),
        "reqflux",
        0x02,
        Behavior::Respond(Arc::new(pong)),
    );
    let mut session = session(dir.path());

    let error = session.client().await.unwrap_err();
    assert!(error.to_string().contains("ui_select_target"), "{error}");

    session.select_target(&second.target_id).await.unwrap();
    assert_eq!(
        session.client().await.unwrap().target_id(),
        second.target_id
    );
    let targets = session.list_targets().await.unwrap();
    let selected: Vec<_> = targets
        .iter()
        .filter(|target| target.selected)
        .map(|target| target.target_id.clone())
        .collect();
    assert_eq!(selected.as_slice(), std::slice::from_ref(&second.target_id));
}

#[tokio::test]
async fn a_selected_target_that_died_is_reported() {
    let dir = TempDir::new().unwrap();
    let first = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );
    let _second = start_in(
        dir.path(),
        "reqflux",
        0x02,
        Behavior::Respond(Arc::new(pong)),
    );
    let mut session = session(dir.path());
    session.select_target(&first.target_id).await.unwrap();
    fs::remove_file(&first.socket_path).unwrap();

    let error = session.client().await.unwrap_err();
    assert!(
        error.to_string().contains(first.target_id.as_str()),
        "{error}"
    );
}

#[tokio::test]
async fn selecting_an_unknown_target_is_an_error() {
    let dir = TempDir::new().unwrap();
    let _bridge = start_in(
        dir.path(),
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(pong)),
    );
    let unknown = TargetId::from_str("reqflux-00000000000000ff").unwrap();
    assert!(session(dir.path()).select_target(&unknown).await.is_err());
}
