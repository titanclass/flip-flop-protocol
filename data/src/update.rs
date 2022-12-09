use core::{cmp::Ordering, str::FromStr};

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
#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PreRelease {
    Alpha(u8),
    Beta(u8),
}
impl Ord for PreRelease {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PreRelease::Alpha(self_ident), PreRelease::Alpha(other_ident)) => {
                self_ident.cmp(other_ident)
            }
            (PreRelease::Alpha(_), PreRelease::Beta(_)) => Ordering::Less,
            (PreRelease::Beta(_), PreRelease::Alpha(_)) => Ordering::Greater,
            (PreRelease::Beta(self_ident), PreRelease::Beta(other_ident)) => {
                self_ident.cmp(other_ident)
            }
        }
    }
}
impl PartialOrd for PreRelease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A compact and limited representation of a version based on
/// https://semver.org. In particular, there is no provision for a
/// build identifier. Also, pre-releases are constrained to Alpha
/// and Beta and must always have an ident.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub pre: Option<PreRelease>,
}
#[derive(Debug)]
pub struct ParseVersionErr;
impl FromStr for Version {
    type Err = ParseVersionErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (l, r) = s.split_once('.').ok_or(ParseVersionErr)?;
        let major = l.parse::<u8>().map_err(|_| ParseVersionErr)?;
        let (l, r) = r.split_once('.').ok_or(ParseVersionErr)?;
        let minor = l.parse::<u8>().map_err(|_| ParseVersionErr)?;
        let (l, r) = if let Some((l, r)) = r.split_once('-') {
            (l, r)
        } else {
            (r, "")
        };
        let patch = l.parse::<u8>().map_err(|_| ParseVersionErr)?;
        let pre = if let Some((_, r)) = r.split_once("alpha.") {
            let ident = r.parse::<u8>().map_err(|_| ParseVersionErr)?;
            Some(PreRelease::Alpha(ident))
        } else if let Some((_, r)) = r.split_once("beta.") {
            let ident = r.parse::<u8>().map_err(|_| ParseVersionErr)?;
            Some(PreRelease::Beta(ident))
        } else {
            None
        };
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then(match (self.pre, other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(self_pre), Some(other_pre)) => self_pre.cmp(&other_pre),
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versions() {
        assert_eq!(
            "1.2.3".parse::<Version>().unwrap(),
            Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: None
            }
        );
        assert_eq!(
            "1.2.3-alpha.1".parse::<Version>().unwrap(),
            Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Some(PreRelease::Alpha(1))
            }
        );
        assert_eq!(
            "1.2.3-beta.1".parse::<Version>().unwrap(),
            Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Some(PreRelease::Beta(1))
            }
        );
    }

    #[test]
    fn test_compare_versions() {
        assert!("1.0.0".parse::<Version>().unwrap() == "1.0.0".parse::<Version>().unwrap());
        assert!("1.0.0".parse::<Version>().unwrap() < "2.0.0".parse::<Version>().unwrap());
        assert!("1.1.0".parse::<Version>().unwrap() < "1.2.0".parse::<Version>().unwrap());
        assert!("1.1.1".parse::<Version>().unwrap() < "1.1.2".parse::<Version>().unwrap());
        assert!("1.0.0-alpha.1".parse::<Version>().unwrap() < "1.0.0".parse::<Version>().unwrap());
        assert!("1.0.0-beta.1".parse::<Version>().unwrap() < "1.0.0".parse::<Version>().unwrap());
        assert!(
            "1.0.0-alpha.1".parse::<Version>().unwrap()
                < "1.0.0-beta.1".parse::<Version>().unwrap()
        );
        assert!(
            "1.0.0-alpha.1".parse::<Version>().unwrap()
                < "1.0.0-alpha.2".parse::<Version>().unwrap()
        );
    }
}
