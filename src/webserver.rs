use std::{
    time::Duration,
    net::SocketAddr,
    collections::HashMap,
    convert::Infallible,
};
use crate::{
    messages,
    stats::StatsCollector,
    static_assets,
};
use slog::{Logger, info, o};
use warp::{self, Filter, Reply, http::StatusCode, Rejection, reply::Response};

use tokio::sync::{
    watch,
    Mutex,
};
use mime_guess;
use headers::{ContentType, HeaderMapExt};

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
    hostname: Option<String>,
    socket: SocketAddr,
    pub state: messages::WorkerState,
    pub elapsed: Option<u128>,
    pub count: u32,
    pub count_1xx: u32,
    pub count_2xx: u32,
    pub count_3xx: u32,
    pub count_4xx: u32,
    pub count_5xx: u32,
}

impl From<messages::Status> for StatsResponse {
    fn from(s: messages::Status) -> StatsResponse {
        StatsResponse {
            hostname: s.hostname,
            socket: s.socket.unwrap(),
            state: s.state,
            elapsed: s.elapsed.map(|e| e.as_millis()),
            count: s.count,
            count_1xx: s.count_1xx,
            count_2xx: s.count_2xx,
            count_3xx: s.count_3xx,
            count_4xx: s.count_4xx,
            count_5xx: s.count_5xx,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AllStatsResponse {
    items: Vec<StatsResponse>
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
    let stats = state.stats.all_stats();
    let r = AllStatsResponse {
        items: stats.into_iter().map(|(_, s)| StatsResponse::from(s)).collect(),
    };
    Ok(warp::reply::json(&r))
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

async fn index() -> Result<impl Reply, Rejection> {
    static_file("/static/index.html".to_string()).await
}

async fn static_file(path: String) -> Result<impl Reply, Rejection> {
    if let Some(c) = static_assets::load_file(&path).await {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let mut r = Response::new(c.into());
        *r.status_mut() = StatusCode::OK;
        r.headers_mut().typed_insert(ContentType::from(mime));
        return Ok(r);
    } else {
        return Err(warp::reject::reject());
    }
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
    let index_page = warp::path::end()
        .and(warp::get())
        .and_then(index);
    let static_file = warp::path!("static" / String)
        .and(warp::get())
        .map(|p: String| "/static/".to_string() + &p)
        .and_then(static_file);

    let workers = warp::path("workers").and(start.or(stop).or(reset));
    let routes = warp::any().and(index_page.or(stats).or(workers).or(static_file));
    info!(logger, "Starting webserver at {}", addr);
    let addr: SocketAddr = addr.parse().unwrap();
    warp::serve(routes).run(addr).await;
    Ok(())
}
