use flip_flop_app::{MandatoryCommands, MandatoryEvents};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum Command {
    NextEvent,
    SomeCommand,
}

impl MandatoryCommands for Command {
    fn next_event() -> Self {
        Self::NextEvent
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Event {
    NoMoreEvents,
    SomeEvent,
}

impl MandatoryEvents for Event {
    fn no_events() -> Self {
        Self::NoMoreEvents
    }
}
