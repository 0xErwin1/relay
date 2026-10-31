//! Content hashes for conflict detection.

use std::fmt;

use sha2::{Digest, Sha256};

/// The SHA-256 of a file's bytes, taken when the file was read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Hashes `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The raw digest.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Why a text is not a [`ContentHash`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a content hash is 64 hexadecimal digits")]
pub struct ParseHashError;

impl std::str::FromStr for ContentHash {
    type Err = ParseHashError;

    /// Parses the 64 lowercase or uppercase hexadecimal digits [`ContentHash`]'s `Display`
    /// writes.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ParseHashError);
        }
        let mut digest = [0_u8; 32];
        for (byte, pair) in digest.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
            let pair = std::str::from_utf8(pair).map_err(|_| ParseHashError)?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| ParseHashError)?;
        }
        Ok(Self(digest))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}
