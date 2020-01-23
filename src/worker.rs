use slog::{
    Logger,
    info,
    debug,
    error,
    o,
    warn,
};
use anyhow::{Result, Error};
use async_tungstenite::tokio::connect_async;
use futures::{
    stream,
    pin_mut,
    StreamExt,
    TryStreamExt,
    sink::SinkExt,
};
use tungstenite::protocol::Message;
use serde_json;
use std::{
    time::{
        Duration,
    },
    sync::Arc,
};
use hyper::{self, Client, Uri};
use hyper_rustls::HttpsConnector;
use rand::{self, seq::SliceRandom};

use tokio::{
    self,
    time,
    runtime,
    sync::{watch, mpsc, oneshot, Mutex},
};
use futures_intrusive::sync::Semaphore;

use crate::stats::Stats;
use crate::messages;

type ClientConnector = HttpsConnector<hyper::client::HttpConnector>;

#[derive(Debug)]
struct State {
    commands: mpsc::Sender<messages::Command>,
    stats: watch::Receiver<messages::Status>,
}

impl State {
    fn new(commands: mpsc::Sender<messages::Command>, stats: watch::Receiver<messages::Status>) -> State {
        State {
            commands,
            stats,
        }
    }
}

async fn handle_message(logger: Logger, msg: Message, mut cmd: mpsc::Sender<messages::Command>) -> Result<bool> {
    let mut exit = false;
    match msg {
        Message::Ping(_) => {
            debug!(logger, "Received Ping");
        },
        Message::Pong(_) => {
            debug!(logger, "Received Pong");
        },
        Message::Text(t) => {
            let m: messages::Command = serde_json::from_str(&t)?;
            let _ = cmd.send(m).await;
        },
        Message::Close(_) => {
            debug!(logger, "Received Close");
            exit = true;
        },
        _ => unimplemented!()
    }
    Ok(exit)
}

#[derive(Debug)]
enum Action {
    Incoming(Result<Message>),
    Outgoing(Message),
    Stats(messages::Status),
}

async fn run(logger: Logger, addr: String, state: State) -> Result<()> {
    info!(logger, "Connecting to {}", addr);
    let url = url::Url::parse(&addr)?;
    debug!(logger, "parsed URL {}", url);
    let res = connect_async(url).await;
    match &res {
        Ok(_) => {},
        Err(e) => {
            error!(logger, "connect error: {:?}", e);
        }
    }
    let (ws_stream, _response) = res?;
    debug!(logger, "Successfully connected");
    let (mut tx, rx) = mpsc::channel(100);
    let (mut outgoing, incoming) = ws_stream.split();
    let handle_incoming = incoming.map_err(|e| e.into()).map(Action::Incoming);
    let handle_outgoing = rx.map(Action::Outgoing);
    let handle_stats = state.stats.map(Action::Stats);
    let s1 = stream::select(handle_stats, handle_incoming);
    let mut combined = stream::select(s1, handle_outgoing);
    loop {
        if let Some(r) = combined.next().await {
            match r {
                Action::Stats(s) => {
                    let _ = tx.send(s.into_message()?).await;
                },
                Action::Incoming(m) => {
                    let exit = match m {
                        Ok(m) => {
                            handle_message(logger.new(o!("handling" => "incoming")), m, state.commands.clone()).await?
                        },
                        Err(e) => {
                            warn!(logger, "Error receiving message: {}", e);
                            true
                        }
                    };
                    if exit {
                        break;
                    }
                },
                Action::Outgoing(m) => {
                    outgoing.send(m).await?;
                }
            }
        }
    }
    Ok(())
}

async fn command_executor(logger: Logger, stats: Stats, mut rx: mpsc::Receiver<messages::Command>) -> Result<()> {
    debug!(logger, "Started executor task");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<bool>();
    let shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));
    let shutdown_rx = Arc::new(Mutex::new(shutdown_rx));
    let mut handle = None;
    while let Some(cmd) = rx.recv().await {
        let shutdown_tx = shutdown_tx.clone();
        info!(logger, "Received command {:?}", cmd);
        match cmd {
            messages::Command::Start { urls, strategy, max_concurrency } => {
                stats.start();
                {
                    let (tx, rx) = oneshot::channel::<bool>();
                    *shutdown_tx.clone().lock().await = Some(tx);
                    *shutdown_rx.clone().lock().await = rx;
                }
                let h = tokio::spawn(task_scheduler(logger.new(o!("task" => "scheduler")), urls, stats.clone(), strategy, max_concurrency, shutdown_rx.clone()));
                handle = Some(h);
            },
            messages::Command::Stop => {
                if stats.current_state() == messages::WorkerState::Busy {
                    debug!(logger, "Sending stop message");
                    shutdown_tx.lock().await.take().map(|l| l.send(false));
                }
                stats.stop();
            },
            messages::Command::Reset => {
                if stats.current_state() == messages::WorkerState::Busy {
                    debug!(logger, "Sending stop message");
                    shutdown_tx.lock().await.take().map(|l| l.send(false));
                }
                stats.reset();
            }
        }
    }
    drop(handle);
    Ok(())
}

struct StrategyChooser {
    idx: usize,
}

impl StrategyChooser {
    fn new() -> StrategyChooser {
        StrategyChooser {
            idx: 0,
        }
    }

    fn choose<'a, T>(&mut self, strategy: messages::AttackStrategy, options: &'a [T]) -> Option<&'a T> {
        match strategy {
            messages::AttackStrategy::Random => {
                options.choose(&mut rand::thread_rng())
            },
            messages::AttackStrategy::InOrder => {
                if options.is_empty() {
                    return None;
                }
                if self.idx >= options.len() {
                    self.idx = 0;
                }
                options.get(self.idx)
            }
        }
    }
}

async fn task_scheduler(logger: Logger, urls: Vec<String>, stats: Stats, strategy: messages::AttackStrategy, max_concurrency: u32, shutdown: Arc<Mutex<oneshot::Receiver<bool>>>) -> Result<()> {
    if urls.is_empty() {
        return Ok(());
    }
    debug!(logger, "Task Scheduler starting");
    let mut chooser = StrategyChooser::new();
    let mut id: u64 = 0;
    let https = HttpsConnector::new();
    let client: Client<_, hyper::Body> = Client::builder().build(https);
    let semaphore = Arc::new(Semaphore::new(false, max_concurrency as usize));
    loop {
        if let Ok(b) = shutdown.lock().await.try_recv() {
            debug!(logger, "Received stop message {}", b);
            break
        }
        semaphore.acquire(1).await.disarm();
        let url: Uri = chooser.choose(strategy, &urls)
            .map(|u| u.parse().map_err(Error::from))
            .unwrap_or_else(|| Err(Error::msg("Couldn't choose url".to_string())))?;

        let t = worker_task(logger.new(o!("task" => "worker")), semaphore.clone(), client.clone(), url, stats.clone(), id);
        id += 1;
        tokio::spawn(async {
            let _ = t.await;
        });
    }
    debug!(logger, "Task Scheduler shutting down");
    Ok(())
}

async fn worker_task(logger: Logger, semaphore: Arc<Semaphore>, client: Client<ClientConnector>, url: Uri, stats: Stats, id: u64) -> Result<u64> {
    let res = client.get(url).await.map_err(|e| Error::msg(format!("{}", e)))?;
    let s = res.status().as_u16();
    stats.record_status(s);
    debug!(logger, "Worker {} result: {}", id, s);
    semaphore.release(1);
    Ok(id)
}

async fn stats_executor(logger: Logger, stats: Stats, tx: watch::Sender<messages::Status>) {
    debug!(logger, "Stats heartbeat starting");
    let timeout = time::interval(Duration::from_secs(5));
    pin_mut!(timeout);
    while let Some(_) = timeout.next().await {
        let s = stats.into_message();
        debug!(logger, "Sending stats => {:?}", &s);
        let _ = tx.broadcast(s);
    }
}

pub fn run_forever(logger: Logger, addr: String) -> Result<()> {
    let mut rt = runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()?;
    let res = rt.block_on(async {
        let (c_tx, c_rx) = mpsc::channel(100);
        let stats = Stats::new();
        let (stats_tx, stats_rx) = watch::channel(stats.into_message());
        let state = State::new(c_tx, stats_rx);
        tokio::spawn(stats_executor(logger.new(o!("task" => "stats")), stats.clone(), stats_tx));
        tokio::spawn(command_executor(logger.new(o!("task" => "executor")), stats.clone(), c_rx));
        tokio::spawn(run(logger.new(o!("task" => "receiver")), addr, state)).await
    });
    res?
}
