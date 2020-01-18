use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tungstenite::protocol::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    pub url: String,
}

impl Command {
    pub fn into_message(&self) -> Result<Message> {
        let s = serde_json::to_string(&self)?;
        Ok(Message::Text(s))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum WorkerState {
    Idle,
    Busy,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    pub state: WorkerState,
    pub elapsed: Option<Duration>,
    pub count: u32,
    pub count_2xx: u32,
    pub count_3xx: u32,
    pub count_4xx: u32,
    pub count_5xx: u32,
}

impl Status {
    pub fn into_message(&self) -> Result<Message> {
        let s = serde_json::to_string(&self)?;
        Ok(Message::Text(s))
    }
}
