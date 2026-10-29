//! Test-only fake bridge: a real Unix socket with an accept loop and a
//! descriptor file, shared by the bridge tests and the `ui_*` tool tests.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use gpui_mcp_protocol::{
    AppId, BridgeResult, Capabilities, EndpointDescriptor, InstanceId, LocalEndpoint, Operation,
    PROTOCOL_VERSION, ProcessId, WireRequest, WireResponse,
};
use interprocess::local_socket::{
    GenericFilePath, ListenerOptions, ToFsName as _, tokio::prelude::*,
};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::task::JoinHandle;

use super::client::BridgeClient;
use super::descriptor::TargetId;
use super::discover::discover;
use super::session::BridgeSession;

/// A process id no process can have: above Linux's `pid_max`.
pub(crate) const DEAD_PID: u32 = 0x7FFF_FFFC;

/// Log of the operations a recording fake bridge received.
pub(crate) type OperationLog = Arc<Mutex<Vec<Operation>>>;

pub(crate) type Responder = Arc<dyn Fn(&WireRequest) -> WireResponse + Send + Sync>;

/// How the fake bridge answers each connection.
pub(crate) enum Behavior {
    /// Answer every request through the responder.
    Respond(Responder),
    /// Read one request, then close the connection without answering.
    Close,
    /// Read one request and never answer.
    Silent,
    /// Read one request, announce a frame above the size cap, then close.
    Oversize,
}

/// A responder that records every operation it answers into `log`.
pub(crate) fn recording(log: OperationLog, responder: Responder) -> Behavior {
    Behavior::Respond(Arc::new(move |request| {
        log.lock()
            .expect("operation log")
            .push(request.operation.clone());
        responder(request)
    }))
}

/// A successful bridge response carrying `result`.
pub(crate) fn result(request: &WireRequest, result: BridgeResult) -> WireResponse {
    WireResponse::success(request.request_id, result)
}

/// A bridge refusal with `message`, for tests of error paths.
pub(crate) fn failure(request: &WireRequest, message: &str) -> WireResponse {
    WireResponse::failure(
        request.request_id,
        gpui_mcp_protocol::BridgeError::new(gpui_mcp_protocol::ErrorCode::Internal, message),
    )
}

pub(crate) struct StartedBridge {
    pub(crate) target_id: TargetId,
    pub(crate) descriptor_path: PathBuf,
    pub(crate) socket_path: PathBuf,
    server: JoinHandle<()>,
}

impl Drop for StartedBridge {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// A fake bridge in its own temporary endpoint directory.
pub(crate) struct FakeBridge {
    pub(crate) dir: TempDir,
    pub(crate) started: StartedBridge,
}

impl FakeBridge {
    pub(crate) fn new(app_id: &str, instance: u64, behavior: Behavior) -> Self {
        let dir = TempDir::new().unwrap();
        let started = start_in(dir.path(), app_id, instance, behavior);
        Self { dir, started }
    }

    pub(crate) async fn client(&self, io_timeout: std::time::Duration) -> BridgeClient {
        let mut targets = discover(self.dir.path(), None).await.unwrap();
        assert_eq!(
            targets.len(),
            1,
            "the fake bridge should be the only target"
        );
        assert_eq!(targets[0].target_id(), self.started.target_id);
        BridgeClient::with_io_timeout(targets.remove(0), io_timeout)
    }

    /// A session looking for `app_id` in this fake's endpoint directory.
    pub(crate) fn session(&self, app_id: &str) -> BridgeSession {
        BridgeSession::new(self.dir.path().to_path_buf(), AppId::new(app_id).unwrap())
    }
}

/// Start a fake bridge inside `dir`: socket, accept loop, descriptor file.
pub(crate) fn start_in(
    dir: &Path,
    app_id: &str,
    instance: u64,
    behavior: Behavior,
) -> StartedBridge {
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
                    let header = u32::try_from(gpui_mcp_protocol::MAX_RESPONSE_BYTES + 1)
                        .unwrap()
                        .to_be_bytes();
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

pub(crate) fn pong(request: &WireRequest) -> WireResponse {
    WireResponse::success(
        request.request_id,
        BridgeResult::Pong {
            app_id: AppId::new("relay").unwrap(),
            pid: ProcessId::new(std::process::id()).unwrap(),
            protocol_version: PROTOCOL_VERSION,
        },
    )
}

pub(crate) fn write_descriptor(
    dir: &Path,
    app_id: &str,
    instance: u64,
    pid: u32,
    socket: &Path,
) -> PathBuf {
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
