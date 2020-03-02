use crate::{
    messages, static_assets,
    stats::{self, StatsCollector},
};
use slog::{info, o, Logger};
use std::{convert::Infallible, net::SocketAddr};
use warp::{self, http::StatusCode, reply::Response, Filter, Rejection, Reply};

use headers::{ContentType, HeaderMapExt};
use mime_guess;
use tokio::sync::{watch, Mutex};

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
    id: u32,
    hostname: Option<String>,
    socket: SocketAddr,
    state: stats::WorkerState,
    latest: Option<SnapshotResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotResponse {
    pub state: stats::WorkerState,
    pub elapsed: Option<u128>,
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
    pub rate_count: f64,
    pub count_1xx: u32,
    pub rate_1xx: f64,
    pub count_2xx: u32,
    pub rate_2xx: f64,
    pub count_3xx: u32,
    pub rate_3xx: f64,
    pub count_4xx: u32,
    pub rate_4xx: f64,
    pub count_5xx: u32,
    pub rate_5xx: f64,
    pub count_fail: u32,
    pub rate_fail: f64,
}

impl From<&stats::Status> for StatsResponse {
    fn from(s: &stats::Status) -> StatsResponse {
        let snapshot = s.snapshots.get(0);
        StatsResponse {
            id: s.id,
            hostname: s.hostname.clone(),
            socket: s.socket,
            state: s.state,
            latest: snapshot.map(|s| s.into()),
        }
    }
}

impl From<&stats::Snapshot> for SnapshotResponse {
    fn from(s: &stats::Snapshot) -> SnapshotResponse {
        SnapshotResponse {
            state: s.state,
            elapsed: s.elapsed.map(|e| e.as_millis()),
            tasks: s.tasks,
            task_queue: s.task_queue,
            tasks_max: s.tasks_max,
            min: s.min,
            max: s.max,
            mean: s.mean,
            stdev: s.stdev,
            median: s.median,
            p90: s.p90,
            count: s.count,
            rate_count: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count) / e)
                .unwrap_or(0.0),
            count_1xx: s.count_1xx,
            rate_1xx: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count_1xx) / e)
                .unwrap_or(0.0),
            count_2xx: s.count_2xx,
            rate_2xx: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count_2xx) / e)
                .unwrap_or(0.0),
            count_3xx: s.count_3xx,
            rate_3xx: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count_3xx) / e)
                .unwrap_or(0.0),
            count_4xx: s.count_4xx,
            rate_4xx: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count_4xx) / e)
                .unwrap_or(0.0),
            count_5xx: s.count_5xx,
            rate_5xx: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count_5xx) / e)
                .unwrap_or(0.0),
            count_fail: s.count_fail,
            rate_fail: s
                .elapsed
                .map(|e| e.as_secs_f64())
                .map(|e| f64::from(s.count_fail) / e)
                .unwrap_or(0.0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AllStatsResponse {
    items: Vec<StatsResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StartCommandRequest {
    urls: Vec<String>,
    strategy: Option<messages::AttackStrategy>,
    max_concurrency: Option<u32>,
}

impl Into<messages::Command> for StartCommandRequest {
    fn into(self) -> messages::Command {
        messages::Command::start(
            self.urls,
            self.strategy.unwrap_or(messages::AttackStrategy::Random),
            self.max_concurrency.unwrap_or(50),
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandResponse {
    command: messages::Command,
}

async fn get_stats(state: State) -> Result<impl Reply, Infallible> {
    let stats = state
        .stats
        .with_stats(|map| map.iter().map(|(_, s)| StatsResponse::from(s)).collect());
    let r = AllStatsResponse { items: stats };
    Ok(warp::reply::json(&r))
}

async fn stop_workers(state: State) -> Result<impl Reply, Infallible> {
    info!(state.logger, "Stopping workers");
    let _ = state
        .command_tx
        .clone()
        .lock()
        .await
        .broadcast(messages::Command::stop());
    Ok(warp::reply::with_status("", StatusCode::NO_CONTENT))
}

async fn reset_workers(state: State) -> Result<impl Reply, Infallible> {
    info!(state.logger, "Resetting workers");
    let _ = state
        .command_tx
        .clone()
        .lock()
        .await
        .broadcast(messages::Command::reset());
    Ok(warp::reply::with_status("", StatusCode::NO_CONTENT))
}

async fn start_workers(state: State, cmd: StartCommandRequest) -> Result<impl Reply, Infallible> {
    info!(state.logger, "Sending command => {:?}", &cmd);
    let c: messages::Command = cmd.into();
    let resp_body = CommandResponse { command: c.clone() };

    let _ = state.command_tx.clone().lock().await.broadcast(c);

    Ok(warp::reply::json(&resp_body))
}

async fn clear_disconnected(state: State) -> Result<impl Reply, Infallible> {
    info!(state.logger, "Pruning disconnected workers");
    state.stats.prune_disconnected();
    Ok(warp::reply::with_status("", StatusCode::NO_CONTENT))
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
        Ok(r)
    } else {
        Err(warp::reject::reject())
    }
}

fn with_state(state: State) -> impl Filter<Extract = (State,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

pub async fn webserver_task(
    logger: Logger,
    addr: String,
    stats: StatsCollector,
    command_tx: watch::Sender<messages::Command>,
) -> TaskResult<()> {
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
        .and(with_state(state.clone()))
        .and_then(reset_workers);
    let clear_disconnected = warp::path("prune")
        .and(warp::post())
        .and(with_state(state.clone()))
        .and_then(clear_disconnected);
    let index_page = warp::path::end().and(warp::get()).and_then(index);
    let static_file = warp::path!("static" / String)
        .and(warp::get())
        .map(|p: String| "/static/".to_string() + &p)
        .and_then(static_file);

    let workers = warp::path("workers").and(start.or(stop).or(reset).or(clear_disconnected));
    let routes = warp::any().and(index_page.or(stats).or(workers).or(static_file));
    info!(logger, "Starting webserver at {}", addr);
    let addr: SocketAddr = addr.parse().unwrap();
    warp::serve(routes).run(addr).await;
    Ok(())
}
