use std::{
    net::SocketAddr,
    collections::HashMap,
    convert::Infallible,
};
use crate::{
    messages,
    stats::StatsCollector
};
use slog::{Logger, info, o};
use warp::{self, Filter, Reply, http::StatusCode};

use tokio::sync::{
    watch,
    Mutex,
};

use anyhow::Result as TaskResult;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct State {
    stats: StatsCollector,
    logger: Logger,
    command_tx: Arc<Mutex<watch::Sender<messages::Command>>>,
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

async fn get_stats(state: State) -> Result<impl Reply, Infallible> {
    Ok(warp::reply::json(&state.stats.all_stats()))
}

async fn stop_workers(state: State) -> Result<impl Reply, Infallible> {
    info!(state.logger, "Stopping workers");
    let _ = state.command_tx.clone().lock().await.broadcast(messages::Command::stop());
    Ok(warp::reply::with_status("", StatusCode::NO_CONTENT))
}

async fn reset_workers(state: State) -> Result<impl Reply, Infallible> {
    info!(state.logger, "Resetting workers");
    let _ = state.command_tx.clone().lock().await.broadcast(messages::Command::reset());
    Ok(warp::reply::with_status("", StatusCode::NO_CONTENT))
}

async fn start_workers(state: State, cmd: StartCommandRequest) -> Result<impl Reply, Infallible> {

    info!(state.logger, "Sending command => {:?}", &cmd);
    let c: messages::Command = cmd.into();
    let resp_body = CommandResponse {command: c.clone()};

    let _ = state.command_tx.clone().lock().await.broadcast(c);

    Ok(warp::reply::json(&resp_body))
}

fn with_state(state: State) -> impl Filter<Extract = (State,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

pub async fn webserver_task(logger: Logger, addr: String, stats: StatsCollector, command_tx: watch::Sender<messages::Command>) -> TaskResult<()> {
    let state = State {
        stats,
        logger: logger.new(o!("task" => "webserver")),
        command_tx: Arc::new(Mutex::new(command_tx)),
    };

    let stats = warp::path("stats")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and_then(get_stats);

    let start = warp::path("start")
        .and(warp::post())
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .and_then(start_workers);

    let stop = warp::path("stop")
        .and(warp::post())
        .and(with_state(state.clone()))
        .and_then(stop_workers);

    let reset = warp::path("reset")
        .and(warp::post())
        .and(with_state(state))
        .and_then(reset_workers);

    let workers = warp::path("workers").and(start.or(stop).or(reset));

    let routes = warp::any().and(stats.or(workers));
    info!(logger, "Starting webserver at {}", addr);
    let addr: SocketAddr = addr.parse().unwrap();
    warp::serve(routes).run(addr).await;
    Ok(())
}
