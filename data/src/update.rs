use heapless::Vec;
use serde::{Deserialize, Serialize};

/// Describes a key for the purposes of update message
/// encryption and authentication.
#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct UpdateKey(pub [u8; 16]);
impl core::fmt::Debug for UpdateKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("UpdateKey").field(&"XXX").finish()
    }
}
#[cfg(feature = "defmt")]
impl defmt::Format for UpdateKey {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "UpdateKey(XXX)");
    }
}

/// A constrained form of pre-release designators along with
/// a numeric identifer.
#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub enum PreRelease {
    Alpha(u32),
    Beta(u32),
}

/// A compact and limited representation of a version based on
/// https://semver.org. In particular, there is no provision for a
/// build identifier. Also, pre-releases are constrained to Alpha
/// and Beta.
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub pre: Option<PreRelease>,
}

/// Prior to sending out an update, the client prepares one or more servers
/// to receive an update. As the client knows the encryption key of
/// a given server, it notifies it of a pending update.
#[derive(Deserialize, Serialize)]
pub struct PrepareForUpdate {
    /// The semantic version of the update. A server can use this to
    /// determine eligibility i.e. update only if greater than what
    /// it already has.
    pub version: Version,
    /// Those server ports that the update applies to. A server uses
    /// a port for a specific function. Thus, if the applicable ports
    /// matches the server's entire capability then it may elect
    /// to be updated. The ports are passed as bits e.g. bit 1 relates
    /// to port 1, bit 3 relates to port 3 and so on.
    pub server_ports: u8,
    /// The [UpdateKey] is generated for a sequence of update messages to
    /// follow and is used by all servers wishing to update based on this
    /// and the version matching.
    pub update_key: UpdateKey,
    /// The number of bytes that comprise the
    /// total update. This allows a server to understand if it has missed
    /// an update message and when it has received all of them.
    pub update_byte_len: u32,
}

/// Update payload for the purposes of a client broadcasting to the
/// servers it has previous shared an update key with. The size of
/// record is determined by the application.
#[derive(Deserialize, Serialize)]
pub struct Update<const N: usize> {
    pub byte_offset: u32,
    /// The update bytes themselves. Cannot exceed 127 bytes.
    pub bytes: Vec<u8, N>,
}

/// The number of bytes in an [Update] that are not part of the
/// `update_bytes` field. Must be used when calculating the size
/// of the update byte vectors in relation to the maximum number
/// of bytes that can be sent.
/// This field presently considers the `update_byte_offset` length
/// and one byte for the length of the `update_bytes` field. `update_bytes`
/// cannot exceed 127 bytes.
pub const UPDATE_BYTES_OVERHEAD: usize = 4 + 1;
