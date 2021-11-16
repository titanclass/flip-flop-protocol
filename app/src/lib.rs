#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../../README.md")]

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Command<'a, T> {
    pub id: T,
    pub data: &'a [u8],
    pub last_event_offset: Option<u32>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Event<'a, T> {
    pub id: T,
    pub offset: u32,
    pub time_delta: u32,
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
            time_delta: 10,
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
                time_delta: 10,
                data: &some_data,
            }
        );
    }
}
