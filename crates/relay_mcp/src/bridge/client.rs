//! Framed client for one bridge target.
//!
//! One request is a 4-byte big-endian length followed by the JSON
//! [`WireRequest`]; a response is framed the same way and must carry the same
//! request id and protocol version. Requests are capped at 1 MiB and
//! responses at 24 MiB, and both directions time out.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gpui_mcp_protocol::{
    BridgeResult, ErrorCode, LocalEndpoint, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Operation,
    PROTOCOL_VERSION, RequestId, WireRequest, WireResponse,
};
use interprocess::local_socket::{
    GenericFilePath, Name, ToFsName as _, tokio::Stream as LocalSocketStream,
    traits::tokio::Stream as _,
};
use thiserror::Error;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::time::timeout;

use super::descriptor::{TargetId, ValidatedTarget};

/// How long a single write or read may take.
pub const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// How long connecting to a target may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Why one bridge call failed. The bridge's own errors surface through
/// [`CallError::Bridge`] with their code and sanitized message.
#[derive(Debug, Error)]
pub enum CallError {
    /// The local endpoint name could not be built.
    #[error("the local endpoint name is invalid")]
    EndpointName,
    /// The endpoint kind does not exist on this platform.
    #[error("local bridge endpoints of this kind are unsupported on this platform")]
    UnsupportedEndpoint,
    /// The socket connection failed.
    #[error("could not connect to the bridge: {0}")]
    Connect(#[source] std::io::Error),
    /// Connecting took longer than the connect timeout.
    #[error("timed out connecting to the bridge")]
    ConnectTimeout,
    /// The request identifier space is exhausted.
    #[error("bridge request identifier space is exhausted")]
    RequestIdExhausted,
    /// The request could not be encoded.
    #[error("could not encode the bridge request: {0}")]
    Encode(#[source] serde_json::Error),
    /// The request exceeds the 1 MiB request bound.
    #[error("bridge request exceeds the 1 MiB request bound")]
    RequestTooLarge,
    /// Writing the request took longer than the I/O timeout.
    #[error("timed out writing the bridge request")]
    WriteTimeout,
    /// The request could not be written.
    #[error("could not write the bridge request: {0}")]
    Write(#[source] std::io::Error),
    /// Waiting for the response took longer than the response timeout.
    #[error("timed out waiting for the bridge response")]
    ResponseTimeout,
    /// The bridge closed the connection before answering.
    #[error("the bridge closed the connection")]
    ConnectionClosed,
    /// The response could not be read.
    #[error("could not read the bridge response: {0}")]
    Read(#[source] std::io::Error),
    /// The announced response size is zero or above the 24 MiB bound.
    #[error("bridge response size is invalid")]
    ResponseSize,
    /// The response ended before its announced length.
    #[error("the bridge response was truncated")]
    Truncated,
    /// The response is not valid JSON.
    #[error("the bridge returned invalid JSON: {0}")]
    Json(#[source] serde_json::Error),
    /// The response names another protocol version.
    #[error("the bridge returned protocol version {0}, but this build speaks {1}")]
    ProtocolMismatch(u16, u16),
    /// The response does not answer this request.
    #[error("the bridge returned a mismatched request id")]
    RequestIdMismatch,
    /// The response has neither a result nor an error.
    #[error("the bridge response has neither a result nor an error")]
    InvalidOutcome,
    /// The bridge refused or failed the operation.
    #[error("{0:?}: {1}")]
    Bridge(ErrorCode, String),
}

/// A client for one validated bridge target.
///
/// Each call opens its own connection, sends one operation with a fresh
/// request id and the target's token, and reads one framed response.
#[derive(Debug)]
pub struct BridgeClient {
    target: ValidatedTarget,
    next_request_id: AtomicU64,
    io_timeout: Duration,
}

impl BridgeClient {
    /// A client for `target` with the production 10 s I/O timeout.
    #[must_use]
    pub fn new(target: ValidatedTarget) -> Self {
        Self {
            target,
            next_request_id: AtomicU64::new(1),
            io_timeout: IO_TIMEOUT,
        }
    }

    /// A client with a custom I/O timeout, for tests.
    #[cfg(test)]
    pub(crate) fn with_io_timeout(target: ValidatedTarget, io_timeout: Duration) -> Self {
        Self {
            target,
            next_request_id: AtomicU64::new(1),
            io_timeout,
        }
    }

    /// The target's opaque, non-secret identity.
    #[must_use]
    pub fn target_id(&self) -> TargetId {
        self.target.target_id()
    }

    /// Send one operation and return the bridge's result.
    ///
    /// # Errors
    ///
    /// Returns a [`CallError`] for every transport or protocol problem and
    /// for the bridge's own errors; it never retries.
    pub async fn call(&self, operation: Operation) -> Result<BridgeResult, CallError> {
        let response_deadline = response_timeout(&operation, self.io_timeout);
        let request_id = RequestId::new(self.next_request_id.fetch_add(1, Ordering::Relaxed))
            .ok_or(CallError::RequestIdExhausted)?;
        let request = WireRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            token: self.target.descriptor().token.clone(),
            operation,
        };
        let payload = serde_json::to_vec(&request).map_err(CallError::Encode)?;
        if payload.len() > MAX_REQUEST_BYTES {
            return Err(CallError::RequestTooLarge);
        }

        let mut stream = connect(&self.target.descriptor().endpoint).await?;
        write_frame(&mut stream, &payload, self.io_timeout).await?;
        let response = read_response(&mut stream, response_deadline, self.io_timeout).await?;

        if response.protocol_version != PROTOCOL_VERSION {
            return Err(CallError::ProtocolMismatch(
                response.protocol_version,
                PROTOCOL_VERSION,
            ));
        }
        if response.request_id != request_id {
            return Err(CallError::RequestIdMismatch);
        }
        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(CallError::Bridge(error.code, error.message)),
            _ => Err(CallError::InvalidOutcome),
        }
    }
}

/// Connect to a bridge endpoint.
pub(crate) async fn connect(endpoint: &LocalEndpoint) -> Result<LocalSocketStream, CallError> {
    let name = endpoint_name(endpoint)?;
    match timeout(CONNECT_TIMEOUT, LocalSocketStream::connect(name)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) => Err(CallError::Connect(error)),
        Err(_) => Err(CallError::ConnectTimeout),
    }
}

fn endpoint_name(endpoint: &LocalEndpoint) -> Result<Name<'_>, CallError> {
    match endpoint {
        LocalEndpoint::Filesystem { path } => path
            .as_path()
            .to_fs_name::<GenericFilePath>()
            .map_err(|_| CallError::EndpointName),
        #[cfg(windows)]
        LocalEndpoint::Namespaced { name } => name
            .as_str()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
            .map_err(|_| CallError::EndpointName),
        #[cfg(not(windows))]
        LocalEndpoint::Namespaced { .. } => Err(CallError::UnsupportedEndpoint),
    }
}

/// How long to wait for a response: the I/O timeout, or the operation's own
/// wait budget plus it, so bounded waits can actually elapse.
fn response_timeout(operation: &Operation, io_timeout: Duration) -> Duration {
    match operation {
        Operation::WaitForTree { timeout_ms, .. }
        | Operation::WaitForFrame { timeout_ms, .. }
        | Operation::ReadMessages {
            wait_ms: timeout_ms,
            ..
        } => Duration::from_millis(*timeout_ms).saturating_add(io_timeout),
        _ => io_timeout,
    }
}

async fn write_frame(
    stream: &mut LocalSocketStream,
    payload: &[u8],
    io_timeout: Duration,
) -> Result<(), CallError> {
    let header = u32::try_from(payload.len())
        .map_err(|_| CallError::RequestTooLarge)?
        .to_be_bytes();
    match timeout(io_timeout, async {
        stream.write_all(&header).await?;
        stream.write_all(payload).await?;
        stream.flush().await
    })
    .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(CallError::Write(error)),
        Err(_) => Err(CallError::WriteTimeout),
    }
}

async fn read_response(
    stream: &mut LocalSocketStream,
    response_deadline: Duration,
    io_timeout: Duration,
) -> Result<WireResponse, CallError> {
    let mut header = [0_u8; 4];
    match timeout(response_deadline, stream.read_exact(&mut header)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(CallError::ConnectionClosed);
        }
        Ok(Err(error)) => return Err(CallError::Read(error)),
        Err(_) => return Err(CallError::ResponseTimeout),
    }
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_RESPONSE_BYTES {
        return Err(CallError::ResponseSize);
    }
    let mut payload = vec![0_u8; length];
    match timeout(io_timeout, stream.read_exact(&mut payload)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(CallError::Truncated);
        }
        Ok(Err(error)) => return Err(CallError::Read(error)),
        Err(_) => return Err(CallError::ResponseTimeout),
    }
    serde_json::from_slice(&payload).map_err(CallError::Json)
}
