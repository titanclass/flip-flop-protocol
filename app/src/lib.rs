#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../../README.md")]

use serde::{Deserialize, Serialize};

/// Mandatory commands define methods that must be provided to obtain
/// all commands that are required by the specification.
pub trait MandatoryCommands {
    /// Returns the command to request the next nearest event
    /// closest to the last_event_offset field of the [CommandRequest].
    fn next_event() -> Self;
}

/// A Command may only be sent by a client, of which there is only one
/// client on the bus. Command requests take a type that provides their
/// command; usually an enum. Command requests convey the last [EventReply]
/// offset that the client has processed for the associated server. The
/// addressing of servers is left to a lower layer e.g. UDP, or a serial-based
/// transport.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct CommandRequest<C: MandatoryCommands> {
    /// The command to issue.
    pub command: C,
    /// The last offset of the server recorded by the client.
    pub last_event_offset: Option<u32>,
}

/// Mandatory events define methods that must be provided to obtain
/// all events that are required by the specification.
pub trait MandatoryEvents {
    /// Returns the event to indicate that there are no more events
    /// available.
    fn no_events() -> Self;
}

/// An EventRequest may only be emitted by a server, of which there can be many, and
/// only in relation to having received a [CommandRequest] from a client. Event requests
/// take a type that provides their identifier; usually an enum. Event requests convey
/// the offset they are associated with. If an offset overflows to zero then it is the
/// server's responsibility to convey any important events that the client may need.
/// It is the client's responsibility to clear state in relation to previous events when
/// an offset less than or equal to the one it requested. An event request also conveys a
/// delta in time in a form that the client and its servers understand, and relative to
/// the server's current notion of time.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct EventReply<E: MandatoryEvents> {
    /// The event to reply.
    pub event: E,
    /// A sequence number to identify an event. Offsets are expected to increment
    /// by one each time. Therefore, it is possible for a client to determine if
    /// there is an event missing and possibly re-request it.
    pub offset: u32,
    /// The age of this event in relation to the server's notion of current time,
    /// expressed in a manner agreed between a client and server e.g. ticks can
    /// represent seconds.
    pub delta_ticks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialisation() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum Command {
            NextEvent,
            SomeOtherCommand,
        }

        impl MandatoryCommands for Command {
            fn next_event() -> Self {
                Command::NextEvent
            }
        }

        let request = CommandRequest {
            command: Command::SomeOtherCommand,
            last_event_offset: None,
        };

        let mut buf = [0; 32];
        assert_eq!(postcard::to_slice(&request, &mut buf).unwrap(), [1, 0]);
        assert_eq!(
            postcard::from_bytes::<CommandRequest<Command>>(&buf).unwrap(),
            CommandRequest {
                command: Command::SomeOtherCommand,
                last_event_offset: None,
            }
        );
    }

    #[test]
    fn test_event_serialisation() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum Event {
            NoMoreEvents,
            SomeOtherEvent,
        }

        impl MandatoryEvents for Event {
            fn no_events() -> Self {
                Event::NoMoreEvents
            }
        }

        let reply = EventReply {
            event: Event::SomeOtherEvent,
            offset: 1,
            delta_ticks: 10,
        };

        let mut buf = [0; 32];
        assert_eq!(
            postcard::to_slice(&reply, &mut buf).unwrap(),
            [1, 1, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            postcard::from_bytes::<EventReply<Event>>(&buf).unwrap(),
            EventReply {
                event: Event::SomeOtherEvent,
                offset: 1,
                delta_ticks: 10,
            }
        );
    }
}
