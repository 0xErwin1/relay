//! Tests for discovery and the framed client, run against a fake bridge
//! listening on a real Unix socket in a temporary endpoint directory.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use gpui_mcp_protocol::{
    AppId, BridgeError, BridgeResult, Capabilities, EndpointDescriptor, ErrorCode, InstanceId,
    LocalEndpoint, MAX_RESPONSE_BYTES, Operation, PROTOCOL_VERSION, ProcessId, RequestId,
    WireRequest, WireResponse,
};
use interprocess::local_socket::{
    GenericFilePath, ListenerOptions, ToFsName as _, tokio::prelude::*,
};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::task::JoinHandle;

use super::client::{BridgeClient, CallError, IO_TIMEOUT};
use super::descriptor::{InvalidTargetId, TargetId};
use super::discover::discover;
use super::session::BridgeSession;

/// A process id no process can have: above Linux's `pid_max`.
const DEAD_PID: u32 = 0x7FFF_FFFC;

/// How long a test client waits for the bridge before giving up. Short tests
/// need a short timeout; production uses the 10 s default.
const TEST_TIMEOUT: Duration = Duration::from_millis(250);

type Responder = Arc<dyn Fn(&WireRequest) -> WireResponse + Send + Sync>;

/// How the fake bridge answers each connection.
enum Behavior {
    /// Answer every request through the responder.
    Respond(Responder),
    /// Read one request, then close the connection without answering.
    Close,
    /// Read one request and never answer.
    Silent,
    /// Read one request, announce a frame above the size cap, then close.
    Oversize,
}

struct StartedBridge {
    target_id: TargetId,
    descriptor_path: PathBuf,
    socket_path: PathBuf,
    server: JoinHandle<()>,
}

impl Drop for StartedBridge {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// A fake bridge in its own temporary endpoint directory.
struct FakeBridge {
    dir: TempDir,
    started: StartedBridge,
}

impl FakeBridge {
    fn new(app_id: &str, instance: u64, behavior: Behavior) -> Self {
        let dir = TempDir::new().unwrap();
        let started = start_in(dir.path(), app_id, instance, behavior);
        Self { dir, started }
    }

    async fn client(&self, io_timeout: Duration) -> BridgeClient {
        let mut targets = discover(self.dir.path(), None).await.unwrap();
        assert_eq!(
            targets.len(),
            1,
            "the fake bridge should be the only target"
        );
        assert_eq!(targets[0].target_id(), self.started.target_id);
        BridgeClient::with_io_timeout(targets.remove(0), io_timeout)
    }
}

/// Start a fake bridge inside `dir`: socket, accept loop, descriptor file.
fn start_in(dir: &Path, app_id: &str, instance: u64, behavior: Behavior) -> StartedBridge {
    let socket = dir.join(format!("{app_id}-{instance:016x}.sock"));
    let listener = ListenerOptions::new()
        .name(socket.as_path().to_fs_name::<GenericFilePath>().unwrap())
        .create_tokio()
        .unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let server = tokio::spawn(serve(listener, behavior));
    let descriptor_path = write_descriptor(dir, app_id, instance, std::process::id(), &socket);
    StartedBridge {
        target_id: TargetId::from_str(&format!("{app_id}-{instance:016x}")).unwrap(),
        descriptor_path,
        socket_path: socket,
        server,
    }
}

async fn serve(listener: LocalSocketListener, behavior: Behavior) {
    loop {
        let Ok(mut stream) = listener.accept().await else {
            return;
        };
        match &behavior {
            Behavior::Close => {
                drop(read_frame(&mut stream).await);
            }
            Behavior::Silent => {
                if read_frame(&mut stream).await.is_some() {
                    std::future::pending::<()>().await;
                }
            }
            Behavior::Oversize => {
                if read_frame(&mut stream).await.is_some() {
                    let header = u32::try_from(MAX_RESPONSE_BYTES + 1).unwrap().to_be_bytes();
                    if stream.write_all(&header).await.is_err() {
                        return;
                    }
                }
            }
            Behavior::Respond(respond) => {
                if let Some(request) = read_frame(&mut stream).await {
                    let payload = serde_json::to_vec(&respond(&request)).unwrap();
                    let header = u32::try_from(payload.len()).unwrap().to_be_bytes();
                    if stream.write_all(&header).await.is_err()
                        || stream.write_all(&payload).await.is_err()
                    {
                        return;
                    }
                }
            }
        }
    }
}

async fn read_frame(stream: &mut LocalSocketStream) -> Option<WireRequest> {
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).await.ok()?;
    let mut payload = vec![0_u8; u32::from_be_bytes(header) as usize];
    stream.read_exact(&mut payload).await.ok()?;
    serde_json::from_slice(&payload).ok()
}

fn pong(request: &WireRequest) -> WireResponse {
    WireResponse::success(
        request.request_id,
        BridgeResult::Pong {
            app_id: AppId::new("relay").unwrap(),
            pid: ProcessId::new(std::process::id()).unwrap(),
            protocol_version: PROTOCOL_VERSION,
        },
    )
}

fn write_descriptor(dir: &Path, app_id: &str, instance: u64, pid: u32, socket: &Path) -> PathBuf {
    let descriptor = EndpointDescriptor {
        protocol_version: PROTOCOL_VERSION,
        app_id: AppId::new(app_id).unwrap(),
        app_name: "Test".to_owned(),
        instance_id: InstanceId::new(instance).unwrap(),
        pid: ProcessId::new(pid).unwrap(),
        endpoint: LocalEndpoint::Filesystem {
            path: socket.to_path_buf(),
        },
        token: "ab".repeat(32),
        native_window_id: None,
        capabilities: Capabilities::default(),
    };
    let path = dir.join(format!("{app_id}-{pid}-{instance:016x}.json"));
    fs::write(&path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

fn descriptor_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_json(dir: &Path, name: &str, value: &serde_json::Value) {
    fs::write(dir.join(name), serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn target_id_round_trips() {
    let id = TargetId::from_parts(
        &AppId::new("relay").unwrap(),
        InstanceId::new(0xff).unwrap(),
    );
    assert_eq!(id.as_str(), "relay-00000000000000ff");
    assert_eq!(id.to_string(), "relay-00000000000000ff");
    assert_eq!(TargetId::from_str("relay-00000000000000ff").unwrap(), id);
    for rejected in [
        "relay",
        "relay-0",
        "relay-00000000000000000",
        "relay-0000000000000000",
        "relay-00000000000000zz",
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
    let _second = start_in(dir.path(), "relay", 0x02, Behavior::Respond(Arc::new(pong)));
    let first = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));

    let targets = discover(dir.path(), None).await.unwrap();
    let ids: Vec<_> = targets
        .iter()
        .map(|target| target.target_id().as_str().to_owned())
        .collect();
    assert_eq!(ids, ["relay-0000000000000001", "relay-0000000000000002"]);

    let other = AppId::new("other").unwrap();
    assert!(discover(dir.path(), Some(&other)).await.unwrap().is_empty());
    drop(first);
}

#[tokio::test]
async fn discovery_skips_unusable_descriptors() {
    let dir = TempDir::new().unwrap();
    let good = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));
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
    let good = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));
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
    let bridge = FakeBridge::new("relay", 0x01, Behavior::Respond(Arc::new(pong)));
    let client = bridge.client(IO_TIMEOUT).await;
    let result = client.call(Operation::Ping).await.unwrap();
    assert!(matches!(result, BridgeResult::Pong { app_id, .. } if app_id.as_str() == "relay"));
}

#[tokio::test]
async fn a_mismatched_request_id_is_an_error() {
    let bridge = FakeBridge::new(
        "relay",
        0x01,
        Behavior::Respond(Arc::new(|request| WireResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId::new(request.request_id.get() + 1).unwrap(),
            result: Some(BridgeResult::Ack),
            error: None,
        })),
    );
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client.call(Operation::Ping).await.unwrap_err();
    assert!(matches!(error, CallError::RequestIdMismatch), "{error}");
}

#[tokio::test]
async fn a_mismatched_protocol_version_is_an_error() {
    let bridge = FakeBridge::new(
        "relay",
        0x01,
        Behavior::Respond(Arc::new(|request| WireResponse {
            protocol_version: 1,
            request_id: request.request_id,
            result: Some(BridgeResult::Ack),
            error: None,
        })),
    );
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client.call(Operation::Ping).await.unwrap_err();
    assert!(
        matches!(error, CallError::ProtocolMismatch(found, expected) if found == 1 && expected == PROTOCOL_VERSION),
        "{error}"
    );
}

#[tokio::test]
async fn a_bridge_error_maps_to_its_code_and_message() {
    let bridge = FakeBridge::new(
        "relay",
        0x01,
        Behavior::Respond(Arc::new(|request| {
            WireResponse::failure(
                request.request_id,
                BridgeError::new(ErrorCode::Unauthorized, "bad token"),
            )
        })),
    );
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client.call(Operation::Ping).await.unwrap_err();
    let CallError::Bridge(code, message) = error else {
        panic!("expected a bridge error, got {error}");
    };
    assert_eq!(code, ErrorCode::Unauthorized);
    assert_eq!(message, "bad token");
}

#[tokio::test]
async fn a_server_that_closes_the_socket_is_an_error() {
    let bridge = FakeBridge::new("relay", 0x01, Behavior::Close);
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client.call(Operation::Ping).await.unwrap_err();
    assert!(matches!(error, CallError::ConnectionClosed), "{error}");
}

#[tokio::test]
async fn an_oversize_response_is_refused() {
    let bridge = FakeBridge::new("relay", 0x01, Behavior::Oversize);
    let client = bridge.client(IO_TIMEOUT).await;
    let error = client.call(Operation::Ping).await.unwrap_err();
    assert!(matches!(error, CallError::ResponseSize), "{error}");
}

#[tokio::test]
async fn a_silent_server_times_out() {
    let bridge = FakeBridge::new("relay", 0x01, Behavior::Silent);
    let client = bridge.client(TEST_TIMEOUT).await;
    let error = client.call(Operation::Ping).await.unwrap_err();
    assert!(matches!(error, CallError::ResponseTimeout), "{error}");
}

// The session: which target the `ui_*` tools talk to (S3, S4).

fn session(dir: &Path) -> BridgeSession {
    BridgeSession::new(dir.to_path_buf(), AppId::new("relay").unwrap())
}

#[tokio::test]
async fn a_session_selects_the_only_target() {
    let dir = TempDir::new().unwrap();
    let bridge = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));
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
    assert!(error.to_string().contains("relay"), "{error}");
}

#[tokio::test]
async fn a_session_asks_which_target_when_several_are_running() {
    let dir = TempDir::new().unwrap();
    let _first = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));
    let second = start_in(dir.path(), "relay", 0x02, Behavior::Respond(Arc::new(pong)));
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
    let first = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));
    let _second = start_in(dir.path(), "relay", 0x02, Behavior::Respond(Arc::new(pong)));
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
    let _bridge = start_in(dir.path(), "relay", 0x01, Behavior::Respond(Arc::new(pong)));
    let unknown = TargetId::from_str("relay-00000000000000ff").unwrap();
    assert!(session(dir.path()).select_target(&unknown).await.is_err());
}
