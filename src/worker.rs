use anyhow::Result;
use async_tungstenite::tokio::connect_async;
use futures::{pin_mut, select, sink::SinkExt, stream, StreamExt, TryStreamExt};
use hyper::{self, Client};
use hyper_rustls::HttpsConnector;
use rand::{self, seq::SliceRandom};
use slog::{debug, error, info, o, warn, Logger};
use std::{
    convert::TryInto,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_stream::wrappers::{ReceiverStream, WatchStream};
use tungstenite::protocol::Message;
use url::Url;

use futures_intrusive::sync::Semaphore;
use tokio::{
    self, runtime,
    sync::{mpsc, oneshot, watch, Mutex},
    time,
};

use crate::messages;
use crate::stats::Stats;

type ClientConnector = HttpsConnector<hyper::client::HttpConnector>;

#[derive(Debug)]
struct State {
    commands: mpsc::Sender<messages::Command>,
    stats: watch::Receiver<messages::Status>,
}

impl State {
    fn new(
        commands: mpsc::Sender<messages::Command>,
        stats: watch::Receiver<messages::Status>,
    ) -> State {
        State { commands, stats }
    }
}

async fn handle_message(
    logger: Logger,
    msg: Message,
    cmd: mpsc::Sender<messages::Command>,
) -> Result<bool> {
    let mut exit = false;
    match msg {
        Message::Ping(_) => {
            debug!(logger, "Received Ping");
        }
        Message::Pong(_) => {
            debug!(logger, "Received Pong");
        }
        Message::Text(t) => {
            let m: messages::Command = serde_json::from_str(&t)?;
            let _ = cmd.send(m).await;
        }
        Message::Close(_) => {
            debug!(logger, "Received Close");
            exit = true;
        }
        _ => unimplemented!(),
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
        Ok(_) => {}
        Err(e) => {
            error!(logger, "connect error: {:?}", e);
        }
    }
    let (ws_stream, _response) = res?;
    debug!(logger, "Successfully connected");
    let (tx, rx) = mpsc::channel(100);
    let (mut outgoing, incoming) = ws_stream.split();
    let handle_incoming = incoming.map_err(|e| e.into()).map(Action::Incoming);
    let handle_outgoing = ReceiverStream::new(rx).map(Action::Outgoing);
    let handle_stats = WatchStream::new(state.stats.clone()).map(Action::Stats);
    let s1 = stream::select(handle_stats, handle_incoming);
    let mut combined = stream::select(s1, handle_outgoing);
    loop {
        if let Some(r) = combined.next().await {
            match r {
                Action::Stats(mut s) => {
                    s.hostname = hostname::get()
                        .map(|h| h.into_string().unwrap_or_else(|_| String::new()))
                        .ok();
                    let s = s.as_message()?;
                    let _ = tx.send(s).await;
                }
                Action::Incoming(m) => {
                    let exit = match m {
                        Ok(m) => {
                            handle_message(
                                logger.new(o!("handling" => "incoming")),
                                m,
                                state.commands.clone(),
                            )
                            .await?
                        }
                        Err(e) => {
                            warn!(logger, "Error receiving message: {}", e);
                            true
                        }
                    };
                    if exit {
                        break;
                    }
                }
                Action::Outgoing(m) => {
                    outgoing.send(m).await?;
                }
            }
        }
    }
    Ok(())
}

async fn command_executor(
    logger: Logger,
    stats: Stats,
    mut rx: mpsc::Receiver<messages::Command>,
) -> Result<()> {
    debug!(logger, "Started executor task");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));
    let shutdown_rx = Arc::new(Mutex::new(shutdown_rx));
    let mut handle = None;
    while let Some(cmd) = rx.recv().await {
        let shutdown_tx = shutdown_tx.clone();
        info!(logger, "Received command {:?}", cmd);
        match cmd {
            messages::Command::Start {
                requests,
                strategy,
                max_concurrency,
            } => {
                stats.start();
                {
                    let (tx, rx) = oneshot::channel::<()>();
                    *shutdown_tx.clone().lock().await = Some(tx);
                    *shutdown_rx.clone().lock().await = rx;
                }
                let h = tokio::spawn(task_scheduler(
                    logger.new(o!("task" => "scheduler")),
                    requests,
                    stats.clone(),
                    strategy,
                    max_concurrency,
                    shutdown_rx.clone(),
                ));
                handle = Some(h);
            }
            messages::Command::Stop => {
                stats.stop();
                if let Some(h) = handle.take() {
                    debug!(logger, "Sending stop message");
                    shutdown_tx.lock().await.take().map(|l| l.send(()));
                    let _ = h.await?;
                }
            }
            messages::Command::Reset => {
                if let Some(h) = handle.take() {
                    debug!(logger, "Sending stop message");
                    shutdown_tx.lock().await.take().map(|l| l.send(()));
                    let _ = h.await?;
                }
                stats.reset();
            }
        }
    }
    drop(handle);
    Ok(())
}

async fn task_scheduler(
    logger: Logger,
    requests: Vec<messages::RequestSpec>,
    mut stats: Stats,
    strategy: messages::AttackStrategy,
    max_concurrency: u32,
    shutdown: Arc<Mutex<oneshot::Receiver<()>>>,
) -> Result<()> {
    if requests.is_empty() {
        return Ok(());
    }
    debug!(logger, "Task Scheduler starting");
    let mut id: u64 = 0;
    let max_batches: u32 = u32::max(max_concurrency, 1);
    let semaphore = Arc::new(Semaphore::new(false, max_batches as usize));
    stats.record_task_max(max_batches);
    info!(logger, "Max Batches: {}", max_batches);
    let https = HttpsConnector::with_native_roots();
    let mut future_list = stream::FuturesUnordered::new();
    loop {
        if shutdown.lock().await.try_recv().is_ok() {
            info!(logger, "Received stop message");
            break;
        }
        let mut stats = stats.clone();
        stats.record_current_tasks(semaphore.permits() as u32);
        stats.record_queue_depth(future_list.len() as u32);
        select! {
            mut s = semaphore.acquire(1) => {
                s.disarm();
                let t1 = worker_task(logger.new(o!("worker" => id)), semaphore.clone(), https.clone(), &requests, strategy, stats, id);
                id = id.wrapping_add(1);
                future_list.push(t1);
            },
            res = future_list.select_next_some() => {
                debug!(logger, "Reaped batch {:?}, permits {}", res, semaphore.permits());
            }
        }
    }
    drop(future_list);
    info!(logger, "Task Scheduler shutting down");
    Ok(())
}

async fn execute_one_request(
    client: &Client<ClientConnector, hyper::Body>,
    request: &messages::RequestSpec,
) -> Result<u16> {
    let url = if let Some(ref field) = request.random_querystring {
        let uuid = uuid::Uuid::new_v4();
        let mut url: Url = request.url.parse()?;
        let query = if let Some(q) = url.query() {
            format!("{}&{}={}", q, field, uuid)
        } else {
            format!("{}={}", field, uuid)
        };
        url.set_query(Some(&query));
        url
    } else {
        request.url.parse::<Url>()?
    };

    let mut req = hyper::Request::builder()
        .uri(url.to_string())
        .version(request.version.into())
        .method::<hyper::Method>(request.method.into());

    for (k, v) in request.headers.iter() {
        req = req.header(k, v);
    }
    if let Some(ref header) = request.random_header {
        let uuid = uuid::Uuid::new_v4();
        req = req.header(header, uuid.to_string());
    }
    let r = if let Some(ref b) = request.body {
        req.body(hyper::Body::from(b.to_string())).unwrap()
    } else {
        req.body(hyper::Body::empty()).unwrap()
    };
    let res = client.request(r).await?;
    Ok(res.status().as_u16())
}

async fn worker_task(
    logger: Logger,
    semaphore: Arc<Semaphore>,
    connector: ClientConnector,
    requests: &[messages::RequestSpec],
    strategy: messages::AttackStrategy,
    mut stats: Stats,
    id: u64,
) -> u64 {
    let http1_client: Client<_, hyper::Body> = Client::builder().build(connector.clone());
    let http2_client: Client<_, hyper::Body> = Client::builder().http2_only(true).build(connector);
    match strategy {
        messages::AttackStrategy::Random => {
            let req = if let Some(r) = requests.choose(&mut rand::thread_rng()) {
                r
            } else {
                error!(logger, "Failed to randomly choose request");
                return id;
            };
            let client = if req.version == messages::HttpVersion::Http2 {
                &http2_client
            } else {
                &http1_client
            };
            let started = Instant::now();
            let status = match execute_one_request(&client, &req).await {
                Ok(s) => Some(s),
                Err(e) => {
                    error!(logger, "{}", e);
                    None
                }
            };
            let elapsed = started.elapsed();
            stats.record(
                status,
                elapsed.as_millis().try_into().unwrap_or(u64::max_value()),
            );
        }
        messages::AttackStrategy::InOrder => {
            for req in requests {
                let client = if req.version == messages::HttpVersion::Http2 {
                    &http2_client
                } else {
                    &http1_client
                };
                let started = Instant::now();
                let status = match execute_one_request(&client, &req).await {
                    Ok(s) => Some(s),
                    Err(e) => {
                        error!(logger, "{}", e);
                        None
                    }
                };
                let elapsed = started.elapsed();
                stats.record(
                    status,
                    elapsed.as_millis().try_into().unwrap_or(u64::max_value()),
                );
            }
        }
    }
    semaphore.release(1);
    id
}

async fn stats_executor(logger: Logger, stats: Stats, tx: watch::Sender<messages::Status>) {
    debug!(logger, "Stats heartbeat starting");
    let timeout = time::interval(Duration::from_secs(5));
    pin_mut!(timeout);
    loop {
        let _ = timeout.tick().await;
        let s = stats.as_message();
        debug!(logger, "Sending stats => {:?}", &s);
        let _ = tx.send(s);
    }
}

pub fn run_forever(logger: Logger, addr: String) -> Result<()> {
    let rt = runtime::Builder::new_multi_thread().enable_all().build()?;
    let res = rt.block_on(async {
        let (c_tx, c_rx) = mpsc::channel(100);
        let stats = Stats::new();
        let (stats_tx, stats_rx) = watch::channel(stats.as_message());
        let state = State::new(c_tx, stats_rx);
        tokio::spawn(stats_executor(
            logger.new(o!("task" => "stats")),
            stats.clone(),
            stats_tx,
        ));
        tokio::spawn(command_executor(
            logger.new(o!("task" => "executor")),
            stats.clone(),
            c_rx,
        ));
        tokio::spawn(run(logger.new(o!("task" => "receiver")), addr, state)).await
    });
    res?
}
