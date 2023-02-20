#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../../README.md")]

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};

/// A Command may only be sent by a client, of which there is only one
/// client on the bus. Command requests take a type that provides their
/// command; usually an enum. Command requests convey the last [EventReply]
/// offset that the client has processed for the associated server, starting at
/// 0 as the default.
/// Note that the addressing of servers is left to a lower layer e.g. UDP, or a
/// serial-based transport.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandRequest<C: DeserializeOwned + Serialize> {
    /// The last offset of the server recorded by the client.
    pub last_event_offset: Option<u32>,
    /// The command to issue, or None if we wish to just get the next event
    /// available.
    #[serde(
        deserialize_with = "deserialise_last_field",
        serialize_with = "serialise_last_field"
    )]
    pub command: Option<C>,
}

/// A temporal event is one that has its durability conveyed.
pub trait TemporalEvent: DeserializeOwned + Serialize {}

/// A type representing that there are no ephemeral events.
pub type NoEE = ();

/// The types of event that can be returned.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub enum EventOf<E, EE> {
    /// An event that has been logged, providing their identifier; usually an enum. These replies convey
    /// the offset they are associated with. If an offset overflows to zero then it is the
    /// server's responsibility to convey any important events that the client may need.
    Logged(E, u32),
    /// An event that has not been logged by the server and may be consumed by the client,
    /// often to convey some instantaneous event that does not need to be recorded. Events
    /// of this category should be benign if they are not consumed by a client.
    Ephemeral(EE),
    /// A recovery event will occur when a requested offset
    /// cannot be returned. The server's existing log start and end offsets
    /// are returned so that a client may determine what
    /// events constitute a recovery of state.
    Recovery(u32, u32),
}
impl<E: Clone + DeserializeOwned + Serialize, EE: Clone + DeserializeOwned + Serialize>
    TemporalEvent for EventOf<E, EE>
{
}

/// An EventRequest may only be emitted by a server, of which there can be many, and
/// only in relation to having received a [CommandRequest] from a client. Event replies
/// take a temporal type that conveys their durability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventReply<E: TemporalEvent> {
    /// The age of this event in relation to the server's notion of current time,
    /// expressed in a manner agreed between a client and server e.g. ticks can
    /// represent seconds.
    pub delta_ticks: u64,
    /// The event to reply.
    #[serde(
        deserialize_with = "deserialise_last_field",
        serialize_with = "serialise_last_field"
    )]
    pub event: Option<E>,
}

/// Given an event and its time, return an event reply containing it.
pub fn event_reply<E, T, DS>(maybe_event: Option<(E, T)>, duration_since: DS) -> EventReply<E>
where
    DS: FnOnce(T) -> u64,
    E: TemporalEvent,
    T: Copy,
{
    // It is quite plausible that we have no events. In this case we
    // reply with a "no more events" enum and delta ticks of 0.
    maybe_event
        .map(|(e, t)| EventReply {
            delta_ticks: duration_since(t),
            event: Some(e),
        })
        .unwrap_or_else(|| EventReply {
            delta_ticks: 0,
            event: None,
        })
}

fn deserialise_last_field<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map_or_else(|_| Ok(None), |v| Ok(Some(v)))
}

fn serialise_last_field<S, T>(o: &Option<T>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    if let Some(o) = o {
        o.serialize(s)
    } else {
        s.serialize_unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialisation_with_a_command() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum Command {
            A,
            B,
            C,
        }

        let request = CommandRequest {
            last_event_offset: Some(9),
            command: Some(Command::C),
        };

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&request, &mut buf).unwrap();
        assert_eq!(serialised, [1, 9, 2]);
        assert_eq!(
            postcard::from_bytes::<CommandRequest<Command>>(serialised).unwrap(),
            CommandRequest {
                last_event_offset: Some(9),
                command: Some(Command::C),
            }
        );
    }

    #[test]
    fn test_command_serialisation_with_no_command() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum Command {
            A,
            B,
        }

        let request = CommandRequest::<Command> {
            last_event_offset: None,
            command: None,
        };

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&request, &mut buf).unwrap();
        assert_eq!(serialised, [0]);
        assert_eq!(
            postcard::from_bytes::<CommandRequest<Command>>(serialised).unwrap(),
            CommandRequest {
                last_event_offset: None,
                command: None,
            }
        );
    }

    #[test]
    fn test_event_serialisation() {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        enum Event {
            A,
            B,
        }

        let reply = event_reply(Some((EventOf::<_, NoEE>::Logged(Event::B, 9), 0)), |_| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [10, 0, 1, 9]);
        assert_eq!(
            postcard::from_bytes::<EventReply<EventOf<Event, NoEE>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 10,
                event: Some(EventOf::Logged(Event::B, 9)),
            }
        );
    }

    #[test]
    fn test_event_serialisation_with_no_more_events() {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        enum Event {
            SomeEvent,
            SomeOtherEvent,
        }

        let reply: EventReply<EventOf<Event, NoEE>> = event_reply(None, |_: i32| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [0]);
        assert_eq!(
            postcard::from_bytes::<EventReply<EventOf<Event, NoEE>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 0,
                event: None,
            }
        );
    }

    #[test]
    fn test_event_serialisation_with_either() {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        enum Event {
            A,
            B,
        }

        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        enum Telemetry {
            A,
        }

        let reply: EventReply<EventOf<Event, Telemetry>> =
            event_reply(Some((EventOf::Logged(Event::B, 9), 0)), |_| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [10, 0, 1, 9]);
        assert_eq!(
            postcard::from_bytes::<EventReply<EventOf<Event, Telemetry>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 10,
                event: Some(EventOf::Logged(Event::B, 9)),
            }
        );

        let reply: EventReply<EventOf<Event, Telemetry>> =
            event_reply(Some((EventOf::Ephemeral(Telemetry::A), 0)), |_| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [10, 1, 0]);
        assert_eq!(
            postcard::from_bytes::<EventReply<EventOf<Event, Telemetry>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 10,
                event: Some(EventOf::Ephemeral(Telemetry::A)),
            }
        );
    }
}
