#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

use crc_any::CRCu8;
use serde::{Deserialize, Serialize};

/// Indicates where data is sourced from.
#[derive(Debug, PartialEq)]
pub enum DataSource {
    Client,
    Server,
}

/// A data frame encapsulates client and server packets
/// and provides for error checking.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct DataFrame<'a> {
    // Bits as follows:
    // 0..=1 protocol version - should be 0
    // 2..=2 source 0 = client, 1 = server
    // 3..=7 server address
    // 8..=9 server port
    // 10..=15 crc
    header: u16,
    data: &'a [u8],
}

/// There was an error parsing the data frame. Possibly due
/// to an incompatible data frame version or the checksum
/// was invalid.
#[derive(Debug, PartialEq)]
pub struct ParseError {}

impl<'a> DataFrame<'a> {
    /// Create a new dataframe. A CRCu8 computer is provided so that the
    /// frame's 6 bit CRC can be determined. Note that any bits beyond
    /// the 6th will be discarded.
    pub fn new(
        mut crc_computer: CRCu8,
        source: DataSource,
        server_address: u8,
        server_port: u8,
        data: &'a [u8],
    ) -> Self {
        let source = if source == DataSource::Client { 0 } else { 1 };
        crc_computer.digest(data);
        let crc = crc_computer.get_crc();
        Self {
            header: (source << 2)
                | (((server_address as u16) & 0x1F) << 3)
                | (((server_port as u16) & 0x03) << 8)
                | ((crc as u16) << 10),
            data,
        }
    }

    /// Parse the contents of the data frame passing in a 6 bit CRC
    /// If the data frame version is an incompatible value or the
    /// CRC check fails then an error is returned. Otherwise, the
    /// data source, server address, server port and data is retu
    pub fn parse(
        &self,
        mut crc_computer: CRCu8,
    ) -> Result<(DataSource, u8, u8, &'a [u8]), ParseError> {
        let version = self.header & 0x02;
        let source = match (self.header >> 2) & 0x01 {
            0 => Some(DataSource::Client),
            1 => Some(DataSource::Server),
            _ => None,
        };
        let server_address = (self.header >> 3) & 0x1f;
        let server_port = (self.header >> 8) & 0x03;
        let header_crc = self.header >> 10;
        crc_computer.digest(self.data);
        let crc = crc_computer.get_crc();

        match (version, source, header_crc as u8 == crc) {
            (0, Some(source), true) => {
                Ok((source, server_address as _, server_port as _, self.data))
            }
            _ => Err(ParseError {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serde() {
        let data = b"some data";

        let expected_frame = DataFrame::new(CRCu8::crc6itu(), DataSource::Server, 31, 2, data);
        assert_eq!(
            expected_frame,
            DataFrame {
                //        FEDCBA_98_76543_2_10
                header: 0b000001_10_11111_1_00_u16,
                data: b"some data",
            }
        );

        let expected_parts: Result<(DataSource, u8, u8, &[u8]), ParseError> =
            Ok((DataSource::Server, 31_u8, 2_u8, data));
        assert_eq!(expected_frame.parse(CRCu8::crc6itu()), expected_parts);
    }
}
