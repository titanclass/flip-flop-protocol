use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum Command {
    SomeCommand,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Event {
    SomeEvent,
}
