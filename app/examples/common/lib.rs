use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum CommandId {
    SomeCommand,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum EventId {
    SomeEvent,
}
