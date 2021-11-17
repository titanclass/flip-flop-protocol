#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../../README.md")]

use serde::{Deserialize, Serialize};

/// A Command may only be sent by a client, of which there is only one
/// client on the bus. Commands take a type that provides their
/// identifier; usually an enum. Commands can provide 0 or more bytes
/// of data and convey the last [Event] offset that the client has processed
/// for the associated server. The addressing of servers is left to a lower
/// layer e.g. UDP, or a serial-based transport.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Command<'a, T> {
    pub id: T,
    pub data: &'a [u8],
    pub last_event_offset: Option<u32>,
}

/// An Event may only be emitted by a server, of which there can be many, and
/// only in relation to having received a [Command] from a client. Events take a type that
/// provides their identifier; usually an enum. Events can provide 0 or more bytes
/// of data and convey the offset they are associated with. If an offset overflows to
/// zero then it is the server's responsibility to convey any important events that
/// the client may need. It is the client's responsibility to clear state in relation
/// to previous events when an offset less than or equal to the one it requested.
/// An event also conveys a delta in time in a form that the client and its servers
/// understand, and relative to the server's current notion of time.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Event<'a, T> {
    pub id: T,
    pub offset: u32,
    pub delta_ticks: u64,
    pub data: &'a [u8],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialisation() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum CommandId {
            Command1,
        }

        let some_data = [13];
        let command = Command {
            id: CommandId::Command1,
            data: &some_data,
            last_event_offset: None,
        };

        let mut buf = [0; 32];
        assert_eq!(
            postcard::to_slice(&command, &mut buf).unwrap(),
            [0, 1, 13, 0]
        );
        assert_eq!(
            postcard::from_bytes::<Command<CommandId>>(&buf).unwrap(),
            Command {
                id: CommandId::Command1,
                data: &some_data,
                last_event_offset: None,
            }
        );
    }

    #[test]
    fn test_event_serialisation() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum EventId {
            Event1,
        }

        let some_data = [13];
        let command = Event {
            id: EventId::Event1,
            offset: 1,
            delta_ticks: 10,
            data: &some_data,
        };

        let mut buf = [0; 32];
        assert_eq!(
            postcard::to_slice(&command, &mut buf).unwrap(),
            [0, 1, 0, 0, 0, 10, 0, 0, 0, 1, 13]
        );
        assert_eq!(
            postcard::from_bytes::<Event<EventId>>(&buf).unwrap(),
            Event {
                id: EventId::Event1,
                offset: 1,
                delta_ticks: 10,
                data: &some_data,
            }
        );
    }
}
