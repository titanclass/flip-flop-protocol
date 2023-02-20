use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::{HEADER_SIZE, MIC_SIZE};

const ADDRESSES_PER_BYTE: usize = 8; // CANNOT CHANGE

/// The maximum number of address we can have on one network. Address 0
/// always represents the client.
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
    pub addresses: [u8; MIN_PAYLOAD_SIZE],
}

/// The payload a server replies with requesting an address
/// to be assigned to.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Identified {
    /// The server address desired by the server.
    pub server_address: u8,
    /// A bit field representing each port supported by
    /// the server e.g. bit 1 represents that port 1 is
    /// supported. The client application can then determine
    /// the type of server being represented given how
    /// each port is to be used.
    pub server_ports: u8,
}

impl Identify {
    /// Returns true if a given address is known to the client.
    pub fn is_address_set(&self, address: u8) -> bool {
        assert!((address as usize) < MAX_ADDRESSES);
        (self.addresses[address as usize / ADDRESSES_PER_BYTE]
            & 1 << (address % (ADDRESSES_PER_BYTE as u8)))
            != 0
    }

    /// An iterator that returns true for addresses known to the client.
    pub fn iter(&self) -> AddressesIter {
        AddressesIter {
            i: 0,
            j: 1,
            addresses: &self.addresses,
        }
    }

    /// Modify the set of addresses known to the client with a new
    /// one.
    pub fn set_address(&mut self, address: u8) {
        assert!((address as usize) < MAX_ADDRESSES);
        self.addresses[address as usize / 8] |= 1 << (address % (ADDRESSES_PER_BYTE as u8));
    }
}

/// An iterator that returns true for each address known
/// to the client.
pub struct AddressesIter<'d> {
    i: usize,
    j: u8,
    addresses: &'d [u8],
}

impl<'d> Iterator for AddressesIter<'d> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < MIN_PAYLOAD_SIZE {
            let item = self.addresses[self.i] & self.j != 0;
            self.j <<= 1;
            if self.j == 0 {
                self.i += 1;
                self.j = 1;
            }
            Some(item)
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
    ///
    /// The `server_ports` parameter is as per the `Identified` structure's field
    /// and conveys a bitmask of ports that are supported by the server. The returned
    /// structure carries this field forward.
    pub fn with_random_address<T>(
        iter: AddressesIter<'_>,
        rng: &mut T,
        server_ports: u8,
    ) -> Option<Self>
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
                server_ports,
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
            addresses: [0; MIN_PAYLOAD_SIZE],
        };
        identify.set_address(1);
        identify.set_address(9);
        assert_eq!(identify.addresses[0], 0b00000010);
        assert_eq!(identify.addresses[1], 0b00000010);
        assert!(identify.is_address_set(1));
        assert!(identify.is_address_set(9));
        assert!(!identify.is_address_set(10));
    }

    #[test]
    fn test_identified_with_none_free() {
        let mut identify = Identify {
            addresses: [0; MIN_PAYLOAD_SIZE],
        };
        for address in 0..MAX_ADDRESSES {
            identify.set_address(address as u8);
        }
        let mut rng_fixture: RngFixture = RngFixture { return_val: 1 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture, 0b00000010),
            None
        );
    }

    #[test]
    fn test_identified_with_one_free() {
        let mut identify = Identify {
            addresses: [0; MIN_PAYLOAD_SIZE],
        };
        for address in 2..MAX_ADDRESSES {
            identify.set_address(address as u8);
        }

        let mut rng_fixture: RngFixture = RngFixture { return_val: 1 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture, 0b00000010),
            Some(Identified {
                server_address: 1,
                server_ports: 0b00000010,
            })
        );
    }

    #[test]
    fn test_identified_with_three_free() {
        let mut identify = Identify {
            addresses: [0; MIN_PAYLOAD_SIZE],
        };
        identify.set_address(0);
        for address in 4..MAX_ADDRESSES {
            identify.set_address(address as u8);
        }

        let mut rng_fixture: RngFixture = RngFixture { return_val: 2 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture, 0b00000010),
            Some(Identified {
                server_address: 3,
                server_ports: 0b00000010,
            })
        );
    }

    #[test]
    fn test_identified_with_all_but_first_free() {
        let mut identify = Identify {
            addresses: [0; MIN_PAYLOAD_SIZE],
        };
        identify.set_address(0);

        let mut rng_fixture: RngFixture = RngFixture { return_val: 254 };
        assert_eq!(
            Identified::with_random_address(identify.iter(), &mut rng_fixture, 0b00000010),
            Some(Identified {
                server_address: 255,
                server_ports: 0b00000010,
            })
        );
    }

    #[test]
    fn test_iter_with_skip() {
        let mut identify = Identify {
            addresses: [0; MIN_PAYLOAD_SIZE],
        };
        identify.set_address(0);
        identify.set_address(3);

        assert_eq!(
            identify
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, is_set)| *is_set),
            Some((3, true))
        );
    }
}
