#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

use serde::{Deserialize, Serialize};

/// Indicates where data is sourced from i.e. its direction.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum DataSource {
    Client,
    Server,
}

/// There was an error parsing the data frame. Possibly due
/// to an incompatible data frame version.
/// was invalid.
#[derive(Debug, PartialEq)]
pub struct ParseError {}

/// The haader fields of the data frame.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Header {
    /// The protocol version. Should be 0.
    pub version: u8,
    /// The direction of data flow.
    pub source: DataSource,
    /// The address of the server 0..31.
    pub server_address: u8,
    /// The port of the server 0..3.
    pub server_port: u8,
    /// A frame counter for ensuring message authenticity by
    /// being able to vary a nonce. Should be incremented by
    /// the message source and is expected to overflow to zero
    /// after 0xFFFF (16 bits).
    pub frame_counter: u16,
}

/// A data frame encapsulates client and server packets
/// and provides for error checking.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct DataFrame<'a> {
    // Bits as follows:
    // 0..=1   protocol version
    // 2..=2   source 0 = client, 1 = server
    // 3..=7   server address
    // 8..=9   server port
    // 10..=15 reserved - must be zero
    // 16..=31 frame counter
    header: u32,
    // Payload data appended with a Message Authentication Code (MAC).
    encrypted_payload: &'a [u8],
}

impl<'a> DataFrame<'a> {
    /// Create a new dataframe with an encrypted payload inclusive of its MAC which
    /// is expected to be appended at the end.
    pub fn new(header: &'a Header, encrypted_payload: &'a [u8]) -> Self {
        let source = if header.source == DataSource::Client {
            0
        } else {
            1
        };
        Self {
            header: (source << 2)
                | (((header.server_address as u32) & 0x1F) << 3)
                | (((header.server_port as u32) & 0x03) << 8)
                | (((header.frame_counter as u32) & 0xFFFF) << 16),
            encrypted_payload,
        }
    }

    /// Parse the contents of the data frame.
    /// If the data frame version is an incompatible value
    /// then an error is returned. Otherwise, the header
    /// and encrypted payload (including a MAC at the end)
    /// are returned.
    pub fn parse(&self) -> Result<(Header, &'a [u8]), ParseError> {
        let version = self.header & 0x02;
        let source = match (self.header >> 2) & 0x01 {
            0 => Some(DataSource::Client),
            1 => Some(DataSource::Server),
            _ => None,
        };
        let server_address = (self.header >> 3) & 0x1F;
        let server_port = (self.header >> 8) & 0x03;
        let frame_counter = (self.header >> 16) & 0xFFFF;

        match (version, source) {
            (0, Some(source)) => Ok((
                Header {
                    version: 0,
                    source,
                    server_address: server_address as _,
                    server_port: server_port as _,
                    frame_counter: frame_counter as _,
                },
                self.encrypted_payload,
            )),
            _ => Err(ParseError {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::Aes128;
    use ccm::aead::AeadInPlace;
    use ccm::aead::{generic_array::GenericArray, NewAead};
    use ccm::{
        consts::{U4, U8},
        Ccm,
    };
    use heapless::Vec;

    #[test]
    fn test_command_serialisation() {
        type AesCcm = Ccm<Aes128, U4, U8>;

        let key = GenericArray::from_slice(b"0123456789ABCDEF");
        let cipher = AesCcm::new(key);

        let header = Header {
            version: 0,
            source: DataSource::Server,
            server_address: 31,
            server_port: 2,
            frame_counter: 1,
        };

        let mut nonce = [0; 8];
        let _ = postcard::to_slice(&header, &mut nonce).unwrap();
        let nonce = GenericArray::from_slice(&nonce);

        let payload = b"some data";
        let mut encrypted_payload: Vec<u8, 128> = Vec::new();
        let _ = encrypted_payload.extend_from_slice(payload).unwrap();
        let _ = cipher
            .encrypt_in_place(nonce, b"", &mut encrypted_payload)
            .unwrap();

        let expected_frame = DataFrame::new(&header, &encrypted_payload);
        assert_eq!(
            expected_frame,
            DataFrame {
                //                        FEDCBA_98_76543_2_10
                header: 0b000000000000001_000000_10_11111_1_00,
                encrypted_payload: &[91, 146, 159, 160, 124, 31, 21, 57, 196, 230, 143, 129, 18],
            }
        );
    }

    #[test]
    fn test_command_deserialisation() {
        type AesCcm = Ccm<Aes128, U4, U8>;

        let key = GenericArray::from_slice(b"0123456789ABCDEF");
        let cipher = AesCcm::new(key);

        let data_frame = DataFrame {
            //                        FEDCBA_98_76543_2_10
            header: 0b000000000000001_000000_10_11111_1_00,
            encrypted_payload: &[91, 146, 159, 160, 124, 31, 21, 57, 196, 230, 143, 129, 18],
        };

        let (header, encrypted_payload) = data_frame.parse().unwrap();

        let expected_header = Header {
            version: 0,
            source: DataSource::Server,
            server_address: 31,
            server_port: 2,
            frame_counter: 1,
        };

        assert_eq!(header, expected_header);

        let mut nonce = [0; 8];
        let _ = postcard::to_slice(&header, &mut nonce).unwrap();
        let nonce = GenericArray::from_slice(&nonce);

        let mut decrypted_payload: Vec<u8, 128> = Vec::new();
        let _ = decrypted_payload
            .extend_from_slice(encrypted_payload)
            .unwrap();
        let _ = cipher
            .decrypt_in_place(nonce, b"", &mut decrypted_payload)
            .unwrap();

        let expected_payload = b"some data";

        assert_eq!(decrypted_payload, expected_payload);
    }
}
