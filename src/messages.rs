use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::{collections::HashMap, time::Duration};
use tungstenite::protocol::Message;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AttackStrategy {
    Random,
    InOrder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RequestMethod {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    CONNECT,
    PATCH,
    TRACE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpVersion {
    Http11,
    Http2,
}

impl From<HttpVersion> for hyper::Version {
    fn from(r: HttpVersion) -> hyper::Version {
        use hyper::Version;
        use HttpVersion::*;
        match r {
            Http11 => Version::HTTP_11,
            Http2 => Version::HTTP_2,
        }
    }
}

impl From<RequestMethod> for hyper::Method {
    fn from(r: RequestMethod) -> hyper::Method {
        use hyper::Method;
        use RequestMethod::*;
        match r {
            GET => Method::GET,
            POST => Method::POST,
            PUT => Method::PUT,
            DELETE => Method::DELETE,
            HEAD => Method::HEAD,
            OPTIONS => Method::OPTIONS,
            CONNECT => Method::CONNECT,
            PATCH => Method::PATCH,
            TRACE => Method::TRACE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSpec {
    pub version: HttpVersion,
    pub method: RequestMethod,
    pub url: String,
    pub body: Option<String>,
    pub headers: HashMap<String, String>,
    pub random_querystring: Option<String>,
    pub random_header: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Stop,
    Start {
        requests: Vec<RequestSpec>,
        strategy: AttackStrategy,
        max_concurrency: u32,
    },
    Reset,
}

impl Command {
    pub fn start(
        requests: Vec<RequestSpec>,
        strategy: AttackStrategy,
        max_concurrency: u32,
    ) -> Command {
        Command::Start {
            requests,
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
    pub tasks: u32,
    pub task_queue: u32,
    pub tasks_max: u32,
    pub min: u64,
    pub max: u64,
    pub mean: f64,
    pub stdev: f64,
    pub median: u64,
    pub p90: u64,
    pub count: u32,
    pub count_1xx: u32,
    pub count_2xx: u32,
    pub count_3xx: u32,
    pub count_4xx: u32,
    pub count_5xx: u32,
    pub count_fail: u32,
}

impl Status {
    pub fn as_message(&self) -> Result<Message> {
        let s = serde_json::to_string(&self)?;
        Ok(Message::Text(s))
    }
}
