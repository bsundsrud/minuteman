use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tungstenite::protocol::Message;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AttackStrategy {
    Random,
    InOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Stop,
    Start {
        urls: Vec<String>,
        strategy: AttackStrategy,
        max_concurrency: u32,
    },
    Reset,
}

impl Command {
    pub fn start(urls: Vec<String>, strategy: AttackStrategy, max_concurrency: u32) -> Command {
        Command::Start {
            urls,
            strategy,
            max_concurrency,
        }
    }

    pub fn stop() -> Command {
        Command::Stop
    }

    pub fn reset() -> Command {
        Command::Reset
    }

    pub fn as_message(&self) -> Result<Message> {
        let s = serde_json::to_string(&self)?;
        Ok(Message::Text(s))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    Idle,
    Busy,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Status {
    pub hostname: Option<String>,
    pub socket: Option<SocketAddr>,
    pub state: WorkerState,
    pub elapsed: Option<Duration>,
    pub min: u64,
    pub max: u64,
    pub mean: f64,
    pub median: u64,
    pub p90: u64,
    pub count: u32,
    pub count_1xx: u32,
    pub count_2xx: u32,
    pub count_3xx: u32,
    pub count_4xx: u32,
    pub count_5xx: u32,
}

impl Status {
    pub fn as_message(&self) -> Result<Message> {
        let s = serde_json::to_string(&self)?;
        Ok(Message::Text(s))
    }
}
