use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize, Deserialize)]
pub struct Heartbeat {
    pub timestamp: SystemTime,
}

impl Heartbeat {
    pub fn new() -> Heartbeat {
        Heartbeat {
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerState {
    Idle,
    Busy,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    state: WorkerState,
    elapsed: Option<Duration>,
    count_2xx: u32,
    count_3xx: u32,
    count_4xx: u32,
    count_5xx: u32,
}
