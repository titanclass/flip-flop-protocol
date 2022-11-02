#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

pub mod discovery;

use aead::{generic_array::GenericArray, AeadInPlace};
use heapless::Vec;
use serde::{Deserialize, Serialize};

/// The size of a data frame header including the byte length for the payload.
/// The byte length value is not to exceed 127.
pub const HEADER_SIZE: usize = 5;

/// The size of the MIC code at the tail of the payload
pub const MIC_SIZE: usize = 4;

/// The size of the Nonce used for encryption
pub const NONCE_SIZE: usize = 7;

/// Indicates where data is sourced from i.e. its direction.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DataSource {
    Client,
    Server,
}

/// There was an error parsing the data frame's header. Possibly due
/// to an incompatible data frame version.
#[derive(Debug, Eq, PartialEq)]
pub struct ParseError {}

/// The haader fields of the data frame.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Header {
    /// The protocol version. Should be 0.
    pub version: u8,
    /// The direction of data flow.
    pub source: DataSource,
    /// The address of the server 0..255.
    pub server_address: u8,
    /// The port of the server 0..7.
    pub server_port: u8,
    /// A frame counter for ensuring message authenticity by
    /// being able to vary a nonce. Should be incremented by
    /// the message source and is expected to overflow to zero
    /// after 0xFFFF (16 bits).
    pub frame_counter: u16,
}

impl Header {
    /// Returns the byte representation of the header.
    pub fn to_packed(&self) -> u32 {
        let source = if self.source == DataSource::Client {
            0
        } else {
            1
        };
        (source << 2)
            | (((self.server_address as u32) & 0xFF) << 3)
            | (((self.server_port as u32) & 0x07) << 11)
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
        let server_address = (header >> 3) & 0xFF;
        let server_port = (header >> 11) & 0x07;
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
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DataFrame<'a> {
    /// Bits as follows:
    /// 00..=01 protocol version 00
    /// 02..=02 source 0 = client, 1 = server
    /// 03..=10 server address
    /// 11..=13 server port
    /// 14..=15 reserved - must be zero
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

/// Conveniently decodes a datagram with a fixed length of N given a condition and,
/// if successful, validates the header and decrypts the payload.
pub fn from_datagram<const N: usize>(
    datagram_buf: &[u8; N],
    filter: impl FnOnce(&Header) -> bool,
    cipher: &impl AeadInPlace,
) -> Option<(Header, Vec<u8, N>)> {
    if let Ok(data_frame) = postcard::from_bytes::<DataFrame>(datagram_buf) {
        if let Ok(header) = Header::parse(data_frame.header) {
            if filter(&header) {
                let nonce = new_nonce(
                    data_frame.header,
                    data_frame.encrypted_payload.len() - MIC_SIZE,
                );

                let mut crypt_payload_buf = Vec::new();
                let _ = crypt_payload_buf.extend_from_slice(data_frame.encrypted_payload);
                if cipher
                    .decrypt_in_place(
                        GenericArray::from_slice(&nonce),
                        &data_frame.header.to_be_bytes(),
                        &mut crypt_payload_buf,
                    )
                    .is_ok()
                {
                    return Some((header, crypt_payload_buf));
                }
            }
        }
    }
    None
}

/// Conveniently encrypts a payload and encodes the header and encrypted payload into
/// a datagram with a fixed length of N.
pub fn to_datagram<const N: usize>(
    cipher: &impl AeadInPlace,
    header: &Header,
    payload_buf: &[u8],
    datagram_buf: &mut [u8; N],
) {
    let packed_header = header.to_packed();
    let header_bytes = packed_header.to_be_bytes();

    let nonce = new_nonce(packed_header, payload_buf.len());

    let mut crypt_payload_buf: Vec<u8, N> = Vec::new();
    crypt_payload_buf.extend_from_slice(payload_buf).unwrap();
    cipher
        .encrypt_in_place(
            GenericArray::from_slice(&nonce),
            &header_bytes,
            &mut crypt_payload_buf,
        )
        .unwrap();

    let data_frame = DataFrame {
        header: packed_header,
        encrypted_payload: &crypt_payload_buf,
    };
    postcard::to_slice(&data_frame, datagram_buf).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    use aead::KeyInit;
    use aes::Aes128;
    use ccm::{
        consts::{U4, U7},
        Ccm,
    };

    #[test]
    fn test_datagram_serialisation() {
        type AesCcm = Ccm<Aes128, U4, U7>;

        let key = GenericArray::from_slice(b"0123456789ABCDEF");
        let cipher = AesCcm::new(key);

        let header = Header {
            version: 0,
            source: DataSource::Server,
            server_address: 255,
            server_port: 7,
            frame_counter: 1,
        };

        let payload_buf = b"some data";
        let mut datagram_buf = [0; 32];
        to_datagram(&cipher, &header, payload_buf, &mut datagram_buf);

        assert_eq!(
            datagram_buf,
            [
                252, 255, 4, 13, 145, 171, 66, 62, 129, 223, 68, 168, 6, 69, 126, 97, 64, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
    }

    #[test]
    fn test_datagram_deserialisation() {
        type AesCcm = Ccm<Aes128, U4, U7>;

        let key = GenericArray::from_slice(b"0123456789ABCDEF");
        let cipher = AesCcm::new(key);

        let datagram_buf = [
            252, 255, 4, 13, 145, 171, 66, 62, 129, 223, 68, 168, 6, 69, 126, 97, 64, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let (header, payload_buf) = from_datagram(
            &datagram_buf,
            |h| h.source == DataSource::Server && h.server_address == 255 && h.server_port == 7,
            &cipher,
        )
        .unwrap();

        assert_eq!(
            header,
            Header {
                version: 0,
                source: DataSource::Server,
                server_address: 255,
                server_port: 7,
                frame_counter: 1,
            }
        );

        assert_eq!(payload_buf, b"some data");
    }
}
