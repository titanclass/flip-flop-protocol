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
///
/// A CommandRequest has the following little endian byte layout:
///
/// | 0 | 1 | 2 | 3 |    ..   |
/// +---+---+---+---+---------+
/// |     offset    | command |
///
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandRequest<C: DeserializeOwned + Serialize> {
    /// The last offset of the server recorded by the client.
    pub last_event_offset: u32,
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

/// An event that has been logged, providing their identifier; usually an enum. These replies convey
/// the offset they are associated with. If an offset overflows to zero then it is the
/// server's responsibility to convey any important events that the client may need.
/// It is the client's responsibility to clear state in relation to previous events when
/// an offset less than or equal to the one it requested is received. An event request also conveys a
/// delta in time in a form that the client and its servers understand, and relative to
/// the server's current notion of time.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Logged<E>(pub E, pub u32);
impl<E: Clone + DeserializeOwned + Serialize> TemporalEvent for Logged<E> {}

/// An event that has not been logged by the server and may be consumed by the client,
/// often to convey some instantaneous event that does not need to be recorded. Events
/// of this category should be benign if they are not consumed by a client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ephemeral<E>(pub E);
impl<E: Clone + DeserializeOwned + Serialize> TemporalEvent for Ephemeral<E> {}

/// Either a logged or an ephemeral event can be declared for use.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub enum Either<E, EE> {
    Logged(E, u32),
    Ephemeral(EE),
}
impl<E: Clone + DeserializeOwned + Serialize, EE: Clone + DeserializeOwned + Serialize>
    TemporalEvent for Either<E, EE>
{
}

/// An EventRequest may only be emitted by a server, of which there can be many, and
/// only in relation to having received a [CommandRequest] from a client. Event replies
/// take a temporal type that conveys their durability.
///
/// An EventReply has the following little endian byte layout:
///
/// | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |  ..   |
/// +---+---+---+---+---+---+---+---+-------+
/// |          delta_ticks          | event |
///
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventReply<E: DeserializeOwned + Serialize + TemporalEvent> {
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
            SomeCommand,
            SomeOtherCommand,
            AndAnotherCommand,
        }

        let request = CommandRequest {
            last_event_offset: 9,
            command: Some(Command::AndAnotherCommand),
        };

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&request, &mut buf).unwrap();
        assert_eq!(serialised, [9, 2]);
        assert_eq!(
            postcard::from_bytes::<CommandRequest<Command>>(serialised).unwrap(),
            CommandRequest {
                last_event_offset: 9,
                command: Some(Command::AndAnotherCommand),
            }
        );
    }

    #[test]
    fn test_command_serialisation_with_no_command() {
        #[derive(Debug, Deserialize, PartialEq, Serialize)]
        enum Command {
            SomeCommand,
            SomeOtherCommand,
        }

        let request = CommandRequest::<Command> {
            last_event_offset: 0,
            command: None,
        };

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&request, &mut buf).unwrap();
        assert_eq!(serialised, [0]);
        assert_eq!(
            postcard::from_bytes::<CommandRequest<Command>>(serialised).unwrap(),
            CommandRequest {
                last_event_offset: 0,
                command: None,
            }
        );
    }

    #[test]
    fn test_event_serialisation() {
        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        enum Event {
            SomeEvent,
            SomeOtherEvent,
        }

        let reply = event_reply(Some((Logged(Event::SomeOtherEvent, 9), 0)), |_| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [10, 1, 9]);
        assert_eq!(
            postcard::from_bytes::<EventReply<Logged<Event>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 10,
                event: Some(Logged(Event::SomeOtherEvent, 9)),
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

        let reply: EventReply<Logged<Event>> = event_reply(None, |_: i32| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [0]);
        assert_eq!(
            postcard::from_bytes::<EventReply<Logged<Event>>>(serialised).unwrap(),
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
            SomeEvent,
            SomeOtherEvent,
        }

        #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
        enum Telemetry {
            SomeTelemetry,
        }

        let reply: EventReply<Either<Event, Telemetry>> =
            event_reply(Some((Either::Logged(Event::SomeOtherEvent, 9), 0)), |_| 10);

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [10, 0, 1, 9]);
        assert_eq!(
            postcard::from_bytes::<EventReply<Either<Event, Telemetry>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 10,
                event: Some(Either::Logged(Event::SomeOtherEvent, 9)),
            }
        );

        let reply: EventReply<Either<Event, Telemetry>> = event_reply(
            Some((Either::Ephemeral(Telemetry::SomeTelemetry), 0)),
            |_| 10,
        );

        let mut buf = [0; 32];
        let serialised = postcard::to_slice(&reply, &mut buf).unwrap();
        assert_eq!(serialised, [10, 1, 0]);
        assert_eq!(
            postcard::from_bytes::<EventReply<Either<Event, Telemetry>>>(serialised).unwrap(),
            EventReply {
                delta_ticks: 10,
                event: Some(Either::Ephemeral(Telemetry::SomeTelemetry)),
            }
        );
    }
}
