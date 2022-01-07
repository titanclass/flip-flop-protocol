#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

use serde::{Deserialize, Serialize};

/// Indicates where data is sourced from i.e. its direction.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum DataSource {
    Client,
    Server,
}

/// There was an error parsing the data frame's header. Possibly due
/// to an incompatible data frame version.
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
    /// The port of the server 0..31.
    pub server_port: u8,
    /// A frame counter for ensuring message authenticity by
    /// being able to vary a nonce. Should be incremented by
    /// the message source and is expected to overflow to zero
    /// after 0xFFFF (16 bits).
    pub frame_counter: u16,
}

impl Header {
    /// Returns the byte representation of the header
    pub fn to_packed(&self) -> u32 {
        let source = if self.source == DataSource::Client {
            0
        } else {
            1
        };
        (source << 2)
            | (((self.server_address as u32) & 0x1F) << 3)
            | (((self.server_port as u32) & 0x1F) << 8)
            | (((self.frame_counter as u32) & 0xFFFF) << 16)
    }

    /// Parse the contents of the data frame header.
    /// If the data frame version is an incompatible value
    /// then an error is returned. Otherwise, the header
    /// and encrypted payload (including a MAC at the end)
    /// are returned.
    pub fn parse(header: u32) -> Result<Header, ParseError> {
        let version = header & 0x02;
        let source = match (header >> 2) & 0x01 {
            0 => Some(DataSource::Client),
            1 => Some(DataSource::Server),
            _ => None,
        };
        let server_address = (header >> 3) & 0x1F;
        let server_port = (header >> 8) & 0x1F;
        let frame_counter = (header >> 16) & 0xFFFF;

        match (version, source) {
            (0, Some(source)) => Ok(Header {
                version: 0,
                source,
                server_address: server_address as _,
                server_port: server_port as _,
                frame_counter: frame_counter as _,
            }),
            _ => Err(ParseError {}),
        }
    }
}

/// A data frame encapsulates client and server packets
/// and provides for error checking.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct DataFrame<'a> {
    /// Bits as follows:
    /// 0..=1   protocol version
    /// 2..=2   source 0 = client, 1 = server
    /// 3..=7   server address
    /// 8..=12  server port
    /// 13..=15 reserved - must be zero
    /// 16..=31 frame counter
    pub header: u32,
    /// Payload data appended with a Message Authentication Code (MAC) using AES-128 CCM
    /// with a 4 byte MIC and a 7 byte nonce derived using the [new_nonce] function.
    /// This will be required to have a one byte length as the first byte.
    pub encrypted_payload: &'a [u8],
}

/// Construct a 7 byte nonce from the header and length of payload.
/// Given that the header contains a frame counter, we should get
/// a reasonable avoidance of the nonce repeating itself. The nonce is
/// laid out as follows:
/// 0..=0   always 0x01
/// 1..=4   packed header in big endian form
/// 5..=5   payload len
/// 6..=6   always 0x00
pub fn new_nonce(header: u32, payload_len: usize) -> [u8; 7] {
    [
        0x01,
        ((header & 0xff000000) >> 24) as u8,
        ((header & 0x00ff0000) >> 16) as u8,
        ((header & 0x0000ff00) >> 8) as u8,
        (header & 0x000000ff) as u8,
        payload_len as u8,
        0x00,
    ]
}

/// The size of a data frame header including the byte length for the payload.
/// The byte length value is not to exceed 127.
pub const HEADER_SIZE: usize = 5;

/// The size of the MIC code at the tail of the payload
pub const MIC_SIZE: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;
    use aes::Aes128;
    use ccm::aead::AeadInPlace;
    use ccm::aead::{generic_array::GenericArray, NewAead};
    use ccm::{
        consts::{U4, U7},
        Ccm,
    };
    use heapless::Vec;

    #[test]
    fn test_command_serialisation() {
        type AesCcm = Ccm<Aes128, U4, U7>;

        let key = GenericArray::from_slice(b"0123456789ABCDEF");
        let cipher = AesCcm::new(key);

        let header = Header {
            version: 0,
            source: DataSource::Server,
            server_address: 31,
            server_port: 2,
            frame_counter: 1,
        };

        let packed_header = header.to_packed();
        let header_bytes = packed_header.to_be_bytes();

        let payload = b"some data";
        let mut encrypted_payload: Vec<u8, 128> = Vec::new();
        let _ = encrypted_payload.extend_from_slice(payload).unwrap();

        let nonce = new_nonce(packed_header, payload.len());

        let _ = cipher
            .encrypt_in_place(
                GenericArray::from_slice(&nonce),
                &header_bytes,
                &mut encrypted_payload,
            )
            .unwrap();

        let expected_frame = DataFrame {
            header: packed_header,
            encrypted_payload: &encrypted_payload,
        };
        assert_eq!(
            expected_frame,
            DataFrame {
                header: 0b000000000000001_000000_10_11111_1_00,
                encrypted_payload: &[74, 164, 23, 189, 104, 81, 155, 24, 180, 35, 193, 13, 149],
            }
        );
    }

    #[test]
    fn test_command_deserialisation() {
        type AesCcm = Ccm<Aes128, U4, U7>;

        let key = GenericArray::from_slice(b"0123456789ABCDEF");
        let cipher = AesCcm::new(key);

        let data_frame = DataFrame {
            header: 0b000000000000001_000000_10_11111_1_00,
            encrypted_payload: &[74, 164, 23, 189, 104, 81, 155, 24, 180, 35, 193, 13, 149],
        };

        let header = Header::parse(data_frame.header).unwrap();

        let expected_header = Header {
            version: 0,
            source: DataSource::Server,
            server_address: 31,
            server_port: 2,
            frame_counter: 1,
        };

        assert_eq!(header, expected_header);

        let nonce = new_nonce(
            data_frame.header,
            data_frame.encrypted_payload.len() - MIC_SIZE,
        );

        let mut decrypted_payload: Vec<u8, 128> = Vec::new();
        let _ = decrypted_payload
            .extend_from_slice(data_frame.encrypted_payload)
            .unwrap();
        let _ = cipher
            .decrypt_in_place(
                GenericArray::from_slice(&nonce),
                &data_frame.header.to_be_bytes(),
                &mut decrypted_payload,
            )
            .unwrap();

        let expected_payload = b"some data";

        assert_eq!(decrypted_payload, expected_payload);
    }
}
