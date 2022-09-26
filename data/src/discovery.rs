use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::{HEADER_SIZE, MIC_SIZE};

const ADDRESSES_PER_BYTE: usize = 8; // CANNOT CHANGE

/// The maximum number of address we can have on one network.
pub const MAX_ADDRESSES: usize = 256;

/// The minimum size of all payloads on the data link layer given
/// the use of discovery.
pub const MIN_PAYLOAD_SIZE: usize = MAX_ADDRESSES / ADDRESSES_PER_BYTE;

/// The minimum size of all packets ((header + payload_len) + payload + MIC)
///  on the data link layer given the use of discovery.
pub const MIN_PACKET_SIZE: usize = HEADER_SIZE + MIN_PAYLOAD_SIZE + MIC_SIZE;

/// The payload broadcast by a client so that servers not
/// present in the known server addresses are able to reply
/// with a requested address.
#[derive(Deserialize, Serialize)]
pub struct Identify {
    pub server_addresses: [u8; MIN_PAYLOAD_SIZE],
}

/// The payload a server replies with requesting an address
/// to be assigned to.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Identified {
    pub server_address: u8,
}

impl Identify {
    /// Returns true if a given address is known to the client.
    pub fn is_server_address_set(&self, server_address: u8) -> bool {
        assert!((server_address as usize) < MAX_ADDRESSES);
        (self.server_addresses[server_address as usize / ADDRESSES_PER_BYTE]
            & 1 << (server_address % (ADDRESSES_PER_BYTE as u8)))
            != 0
    }

    /// An iterator that returns true for addresses known to the client.
    pub fn iter(&self) -> ServerAddressesIter {
        ServerAddressesIter {
            i: 0,
            j: 1,
            server_addresses: &self.server_addresses,
        }
    }

    /// Modify the set of addresses known to the client with a new
    /// one.
    pub fn set_server_address(&mut self, server_address: u8) {
        assert!((server_address as usize) < MAX_ADDRESSES);
        self.server_addresses[server_address as usize / 8] |=
            1 << (server_address % (ADDRESSES_PER_BYTE as u8));
    }
}

/// An iterator that returns true for each server address known
/// to the client. Note that address 0 is reserved by the client
/// for broadcasting and so will always return true.
pub struct ServerAddressesIter<'d> {
    i: usize,
    j: u8,
    server_addresses: &'d [u8],
}

impl<'d> Iterator for ServerAddressesIter<'d> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < MIN_PAYLOAD_SIZE {
            if self.i == 0 && self.j == 1 {
                self.j = 2;
                Some(true)
            } else {
                let item = self.server_addresses[self.i] & self.j != 0;
                self.j <<= 1;
                if self.j == 0 {
                    self.i += 1;
                    self.j = 1;
                }
                Some(item)
            }
        } else {
            None
        }
    }
}

impl Identified {
    /// Attempt to determine an address given the addresses known to a client and
    /// a random number generator. The function guarantees that no existing address
    /// is returned, and randomly picks an address with the ones that remain to be
    /// known to the client. A return value of None signals that no address can be
    /// found. This can happen if there are no addresses left to be allocated.
    pub fn with_random_address<T>(iter: ServerAddressesIter<'_>, rng: &mut T) -> Option<Self>
    where
        T: RngCore,
    {
        let mut spare_addresses = [0; MAX_ADDRESSES];
        let mut j = 0;
        for (i, taken) in iter.enumerate() {
            if !taken {
                spare_addresses[j] = i;
                j += 1;
            }
        }
        if j > 0 {
            let j = (rng.next_u32() % (j as u32)) as usize;
            Some(Self {
                server_address: spare_addresses[j] as u8,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RngFixture {
        return_val: u32,
    }

    impl RngCore for RngFixture {
        fn next_u32(&mut self) -> u32 {
            self.return_val
        }

        fn next_u64(&mut self) -> u64 {
            todo!()
        }

        fn fill_bytes(&mut self, _dest: &mut [u8]) {
            todo!()
        }

        fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand::Error> {
            todo!()
        }
    }

    #[test]
    fn test_set_get_bits() {
        let mut identify = Identify {
            server_addresses: [0; MIN_PAYLOAD_SIZE],
        };
        identify.set_server_address(1);
        identify.set_server_address(9);
        assert_eq!(identify.server_addresses[0], 0b00000010);
        assert_eq!(identify.server_addresses[1], 0b00000010);
        assert!(identify.is_server_address_set(1));
        assert!(identify.is_server_address_set(9));
        assert!(!identify.is_server_address_set(10));
    }

    #[test]
    fn test_identified_with_none_free() {
        let mut identify = Identify {
            server_addresses: [0; MIN_PAYLOAD_SIZE],
        };
        for server_address in 0..MAX_ADDRESSES {
            identify.set_server_address(server_address as u8);
        }
        let mut rng_fixture: RngFixture = RngFixture { return_val: 1 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture),
            None
        );
    }

    #[test]
    fn test_identified_with_one_free() {
        let mut identify = Identify {
            server_addresses: [0; MIN_PAYLOAD_SIZE],
        };
        for server_address in 2..MAX_ADDRESSES {
            identify.set_server_address(server_address as u8);
        }

        let mut rng_fixture: RngFixture = RngFixture { return_val: 1 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture),
            Some(Identified { server_address: 1 })
        );
    }

    #[test]
    fn test_identified_with_three_free() {
        let mut identify = Identify {
            server_addresses: [0; MIN_PAYLOAD_SIZE],
        };
        for server_address in 4..MAX_ADDRESSES {
            identify.set_server_address(server_address as u8);
        }

        let mut rng_fixture: RngFixture = RngFixture { return_val: 2 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture),
            Some(Identified { server_address: 3 })
        );
    }

    #[test]
    fn test_identified_with_all_but_first_free() {
        let identify = Identify {
            server_addresses: [0; MIN_PAYLOAD_SIZE],
        };

        let mut rng_fixture: RngFixture = RngFixture { return_val: 254 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture),
            Some(Identified {
                server_address: 255
            })
        );
    }
}
