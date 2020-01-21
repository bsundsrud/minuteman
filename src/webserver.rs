use async_std::sync::Sender;
use std::{
    net::SocketAddr,
    collections::HashMap,
};
use crate::{
    messages,
    stats::StatsCollector
};
use slog::{Logger, info, debug, o};
use tide::{Request, Response, Server};
use anyhow::Result as TaskResult;

use serde::{Deserialize, Serialize};


struct State {
    stats: StatsCollector,
    logger: Logger,
    command_tx: Sender<messages::Command>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatsResponse {
    items: HashMap<SocketAddr, messages::Status>
}

#[derive(Debug, Serialize, Deserialize)]
struct StartCommandRequest {
    urls: Vec<String>,
    strategy: Option<messages::AttackStrategy>,
    max_concurrency: Option<u32>
}

impl Into<messages::Command> for StartCommandRequest {
    fn into(self) -> messages::Command {
        messages::Command::start(self.urls,
                                 self.strategy.unwrap_or(messages::AttackStrategy::Random),
                                 self.max_concurrency.unwrap_or(50))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandResponse {
    command: messages::Command,
}

async fn get_stats(req: Request<State>) -> Response {
    match req.state().stats.serialize_all() {
        Ok(r) => Response::new(200).body_string(r),
        Err(e) => Response::new(500).body_string(format!("{}", e))
    }
}

async fn stop_workers(req: Request<State>) -> Response {
    let logger = &req.state().logger;
    info!(logger, "Stopping workers");
    let command_tx = &req.state().command_tx;
    command_tx.send(messages::Command::stop()).await;
    Response::new(204)
}

async fn reset_workers(req: Request<State>) -> Response {
    let logger = &req.state().logger;
    info!(logger, "Resetting workers");
    let command_tx = &req.state().command_tx;
    command_tx.send(messages::Command::reset()).await;
    Response::new(204)
}

async fn start_workers(mut req: Request<State>) -> Response {
    let command: StartCommandRequest = match req.body_json().await {
        Ok(c) => c,
        Err(e) => return Response::new(400).body_string(format!("Bad Request: {}", e)),
    };
    let logger = &req.state().logger;
    info!(logger, "Sending command => {:?}", &command);
    let command_tx = &req.state().command_tx;
    let c: messages::Command = command.into();
    let resp_body = CommandResponse {command: c.clone()};

    command_tx.send(c).await;

    match Response::new(200).body_json(&resp_body) {
        Ok(val) => val,
        Err(e) => Response::new(500).body_string(format!("Error Serializing Response: {}", e)),
    }
}

pub async fn webserver_task(logger: Logger, addr: String, stats: StatsCollector, command_tx: Sender<messages::Command>) -> TaskResult<()> {
    let mut app = Server::with_state(State {
        stats,
        logger: logger.new(o!("task" => "webserver")),
        command_tx,
    });

    app.at("/stats").get(get_stats);
    app.at("/workers/start").post(start_workers);
    app.at("/workers/stop").post(stop_workers);
    app.at("/workers/reset").post(reset_workers);
    info!(logger, "Starting webserver at {}", addr);
    app.listen(addr).await?;
    Ok(())
}
