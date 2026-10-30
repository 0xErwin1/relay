//! Endpoint descriptor reading, validation, and target identities.
//!
//! The bridge writes one descriptor file per window under the runtime
//! directory. A descriptor is trusted only after every rule here passes: it
//! must be a small, owner-only, regular file describing a live process and an
//! owner-only Unix socket next to it.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use gpui_mcp_protocol::{
    AppId, Capabilities, Capability, EndpointDescriptor, InstanceId, LocalEndpoint,
    PROTOCOL_VERSION,
};
use serde::Serialize;
use thiserror::Error;

/// Largest accepted endpoint descriptor file, in bytes.
pub const MAX_DESCRIPTOR_BYTES: u64 = 16 * 1024;

/// Exact length of the endpoint token: 64 hex characters.
const TOKEN_LENGTH: usize = 64;

/// Why a descriptor file or its endpoint was rejected.
#[derive(Debug, Error)]
pub enum DescriptorError {
    /// The descriptor file could not be read.
    #[error("could not read endpoint descriptor {1}: {0}")]
    Io(#[source] std::io::Error, PathBuf),
    /// The descriptor is not a small, regular, non-symlink file.
    #[error("endpoint descriptor must be a regular, non-symlink file of at most 16 KiB")]
    NotPlainFile,
    /// The descriptor is readable or writable by group or other users.
    #[error("refusing endpoint descriptor readable or writable by group or other users")]
    DescriptorPermissions,
    /// The descriptor is not valid JSON.
    #[error("endpoint descriptor contains invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The descriptor speaks another protocol version.
    #[error("endpoint protocol is {found}, but this build speaks {expected}")]
    ProtocolVersion { found: u16, expected: u16 },
    /// The token is not 64 hex characters.
    #[error("endpoint authentication token must be 64 hex characters")]
    TokenShape,
    /// The endpoint is not an absolute socket inside the descriptor directory.
    #[error("filesystem local endpoint must be an absolute socket in the descriptor directory")]
    EndpointDirectory,
    /// The endpoint is not a Unix-domain socket.
    #[error("filesystem local endpoint is not a Unix-domain socket")]
    EndpointNotSocket,
    /// The endpoint is not owner-only.
    #[error("filesystem local endpoint must be accessible by its owner only")]
    EndpointPermissions,
    /// The endpoint could not be inspected.
    #[error("could not inspect the filesystem local endpoint: {0}")]
    EndpointIo(#[source] std::io::Error),
    /// The endpoint kind does not exist on this platform.
    #[error("local bridge endpoints of this kind are unsupported on this platform")]
    UnsupportedEndpoint,
    /// The descriptor advertises screenshots without a native window id.
    #[error("endpoint advertises screenshots without a native window identifier")]
    ScreenshotWithoutWindow,
}

/// Opaque, validated identity of one bridge instance: `<app_id>-<instance>`,
/// where the instance is 16 lowercase hex digits.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TargetId(String);

impl TargetId {
    pub(crate) fn from_parts(app_id: &AppId, instance_id: InstanceId) -> Self {
        Self(format!("{}-{:016x}", app_id, instance_id.get()))
    }

    /// Return the target id as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for TargetId {
    type Err = InvalidTargetId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((app_id, instance)) = value.rsplit_once('-') else {
            return Err(InvalidTargetId);
        };
        AppId::new(app_id).map_err(|_| InvalidTargetId)?;
        if instance.len() != 16 || !instance.bytes().all(is_lower_hex) {
            return Err(InvalidTargetId);
        }
        let instance = u64::from_str_radix(instance, 16)
            .ok()
            .and_then(InstanceId::new)
            .ok_or(InvalidTargetId)?;
        let _ = instance;
        Ok(Self(value.to_owned()))
    }
}

/// Whether `byte` is a lowercase hexadecimal digit. The bridge writes its token and instance
/// id in lowercase; anything else is a descriptor Relay does not accept.
fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// A target id that does not match `<valid app id>-<16 hex digits>`.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("target ID must be <app_id>-<16 hex digits> with a nonzero instance identifier")]
pub struct InvalidTargetId;

/// One descriptor that passed every validation rule.
#[derive(Clone, Debug)]
pub struct ValidatedTarget {
    pub(crate) descriptor: EndpointDescriptor,
}

impl ValidatedTarget {
    /// The validated descriptor behind this target.
    #[must_use]
    pub fn descriptor(&self) -> &EndpointDescriptor {
        &self.descriptor
    }

    /// The target's opaque, non-secret identity.
    #[must_use]
    pub fn target_id(&self) -> TargetId {
        TargetId::from_parts(&self.descriptor.app_id, self.descriptor.instance_id)
    }

    /// The public summary the `ui_list_targets` tool returns.
    #[must_use]
    pub fn summary(&self) -> TargetSummary {
        TargetSummary::from_descriptor(&self.descriptor)
    }
}

/// Public, non-secret identity of one discoverable bridge, as the `ui_*`
/// tools report it.
#[derive(Clone, Debug, Serialize)]
pub struct TargetSummary {
    /// The target's opaque identity.
    pub target_id: TargetId,
    /// Validated application identifier.
    pub app_id: String,
    /// Human-readable application name.
    pub app_name: String,
    /// Process identifier that owns the endpoint.
    pub pid: u32,
    /// Supported operations.
    pub capabilities: Capabilities,
    /// Whether the session's `ui_*` tools talk to this target.
    pub selected: bool,
}

impl TargetSummary {
    pub(crate) fn from_descriptor(descriptor: &EndpointDescriptor) -> Self {
        Self {
            target_id: TargetId::from_parts(&descriptor.app_id, descriptor.instance_id),
            app_id: descriptor.app_id.as_str().to_owned(),
            app_name: descriptor.app_name.clone(),
            pid: descriptor.pid.get(),
            capabilities: descriptor.capabilities.clone(),
            selected: false,
        }
    }
}

/// Read a descriptor file and validate it against every rule.
///
/// # Errors
///
/// Returns a [`DescriptorError`] naming the first rule the file breaks.
pub fn read_and_validate(path: &Path) -> Result<ValidatedTarget, DescriptorError> {
    let descriptor = read_descriptor(path)?;
    validate(&descriptor, path)?;
    Ok(ValidatedTarget { descriptor })
}

pub(crate) fn read_descriptor(path: &Path) -> Result<EndpointDescriptor, DescriptorError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| DescriptorError::Io(error, path.to_path_buf()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_DESCRIPTOR_BYTES
    {
        return Err(DescriptorError::NotPlainFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(DescriptorError::DescriptorPermissions);
        }
    }
    let bytes = fs::read(path).map_err(|error| DescriptorError::Io(error, path.to_path_buf()))?;
    serde_json::from_slice(&bytes).map_err(DescriptorError::Json)
}

pub(crate) fn validate(
    descriptor: &EndpointDescriptor,
    descriptor_path: &Path,
) -> Result<(), DescriptorError> {
    if descriptor.protocol_version != PROTOCOL_VERSION {
        return Err(DescriptorError::ProtocolVersion {
            found: descriptor.protocol_version,
            expected: PROTOCOL_VERSION,
        });
    }
    if descriptor.token.len() != TOKEN_LENGTH || !descriptor.token.bytes().all(is_lower_hex) {
        return Err(DescriptorError::TokenShape);
    }
    if descriptor.capabilities.supports(Capability::Screenshot)
        && descriptor.native_window_id.is_none()
    {
        return Err(DescriptorError::ScreenshotWithoutWindow);
    }
    validate_endpoint(&descriptor.endpoint, descriptor_path)
}

#[cfg(unix)]
fn validate_endpoint(
    endpoint: &LocalEndpoint,
    descriptor_path: &Path,
) -> Result<(), DescriptorError> {
    use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};

    let LocalEndpoint::Filesystem { path } = endpoint else {
        return Err(DescriptorError::UnsupportedEndpoint);
    };
    if !path.is_absolute() {
        return Err(DescriptorError::EndpointDirectory);
    }
    let descriptor_dir = descriptor_path
        .parent()
        .ok_or(DescriptorError::EndpointDirectory)?
        .canonicalize()
        .map_err(DescriptorError::EndpointIo)?;
    let endpoint_dir = path
        .parent()
        .ok_or(DescriptorError::EndpointDirectory)?
        .canonicalize()
        .map_err(DescriptorError::EndpointIo)?;
    if descriptor_dir != endpoint_dir {
        return Err(DescriptorError::EndpointDirectory);
    }
    let socket = fs::symlink_metadata(path).map_err(DescriptorError::EndpointIo)?;
    if socket.file_type().is_symlink() || !socket.file_type().is_socket() {
        return Err(DescriptorError::EndpointNotSocket);
    }
    if socket.permissions().mode() & 0o077 != 0 {
        return Err(DescriptorError::EndpointPermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_endpoint(
    endpoint: &LocalEndpoint,
    descriptor_path: &Path,
) -> Result<(), DescriptorError> {
    let _ = (endpoint, descriptor_path);
    Err(DescriptorError::UnsupportedEndpoint)
}
