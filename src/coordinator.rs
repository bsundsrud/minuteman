use slog::{
    Logger,
    debug,
    info,
    warn,
    o,
};
use anyhow::Result;
use async_tungstenite::{
    self,
    tokio::TokioAdapter,
};
use tungstenite::protocol::Message;
use std::{
    net::SocketAddr,
    time::Duration,
};
use crate::{
    messages,
    stats::StatsCollector,
    webserver,
};

use serde_json;

use futures::{
    stream::{self, StreamExt, TryStreamExt},
    sink::SinkExt,
    pin_mut,
};

use tokio::{
    self,
    net::{
        TcpStream,
        TcpListener,
    },
    time,
    sync::{watch, mpsc, oneshot},
};

#[derive(Clone, Debug)]
struct State {
    //peer_map: PeerMap,
    heartbeat: watch::Receiver<()>,
    broadcast: watch::Receiver<messages::Command>,
    stats: mpsc::Sender<(SocketAddr, messages::Status)>,
}

impl State {
    fn new(heartbeat: watch::Receiver<()>, broadcast: watch::Receiver<messages::Command>, stats: mpsc::Sender<(SocketAddr, messages::Status)>) -> State {
        State {
            heartbeat,
            broadcast,
            stats,
        }
    }
}

pub async fn heartbeat_task(logger: Logger, sender: watch::Sender<()>, shutdown: oneshot::Receiver<()>, period: Duration) {
    debug!(logger, "Heartbeat starting");
    let timeout = time::interval(period);
    pin_mut!(shutdown, timeout);
    loop {
        timeout.tick().await;
        if let Ok(_) = shutdown.try_recv() {
            debug!(logger, "Heartbeat task exiting");
            return
        }
        debug!(logger, "Sending heartbeat");
        if let Err(e) = sender.broadcast(()) {
            info!(logger, "Heartbeat channel error: {}", e);
            return
        }
    }
}

pub async fn start(logger: Logger, addr: String, broadcast: watch::Receiver<messages::Command>, shutdown: oneshot::Receiver<()>, stats: mpsc::Sender<(SocketAddr, messages::Status)>) -> Result<()> {
    debug!(logger, "Starting coordinator");
    let (hb_tx, hb_rx) = watch::channel(());
    tokio::spawn(heartbeat_task(logger.new(o!("task" => "heartbeat")), hb_tx, shutdown, Duration::from_secs(5)));

    let state = State::new(hb_rx, broadcast, stats);
    let mut listener = TcpListener::bind(&addr).await?;
    info!(logger, "Listening on {}", &addr);
    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(handle_connection(logger.new(o!("client" => addr)), state.clone(), stream, addr));
    }
}

async fn handle_incoming_message(log: Logger, msg: Message, mut stats: mpsc::Sender<(SocketAddr, messages::Status)>, addr: &SocketAddr) -> Result<bool> {
    let mut exit = false;
    match msg {
        Message::Ping(_) => {
            debug!(log, "Received ping");
        },
        Message::Pong(_) => {
            debug!(log, "Received pong");
        },
        Message::Text(t) => {
            let m: messages::Status = serde_json::from_str(&t)?;
            debug!(log, "Received Status => {:?}", m);
            let _ = stats.send((addr.clone(), m)).await;
        },
        Message::Close(_) => {
            exit = true;
            debug!(log, "Received close");
        },
        _ => unimplemented!()
    }
    Ok(exit)
}
#[derive(Debug)]
enum CoordinatorResult {
    Incoming(Result<Message>),
    Broadcast(messages::Command),
    Heartbeat,
    Outgoing(Message),
}

async fn handle_connection(logger: Logger, state: State, raw_stream: TcpStream, addr: SocketAddr) -> Result<()> {
    debug!(logger, "Client connected");
    let ws_stream = async_tungstenite::accept_async(TokioAdapter(raw_stream)).await?;
    info!(logger, "WebSocket connection established");
    let (mut tx, rx) = mpsc::channel(100);

    let (mut outgoing, incoming) = ws_stream.split();
    let handle_incoming = incoming
        .map_err(|e| e.into())
        .map(|m| CoordinatorResult::Incoming(m));

    let handle_broadcast = state.broadcast
        .map(|c| CoordinatorResult::Broadcast(c));
    let handle_heartbeat = state.heartbeat
        .map(|_| CoordinatorResult::Heartbeat);

    let responder = rx
        .map(|m| CoordinatorResult::Outgoing(m));

    let s1 = stream::select(handle_heartbeat, handle_broadcast);
    let s2 = stream::select(handle_incoming, responder);
    let mut combined = stream::select(s1, s2);
    while let Some(r) = combined.next().await {
        debug!(logger, "Action: {:?}", r);
        match r {
            CoordinatorResult::Heartbeat => {
                let _ = tx.send(Message::Ping(Vec::new())).await;
            },
            CoordinatorResult::Incoming(m) => {
                let stats = state.stats.clone();
                let exit = match m {
                    Ok(m) => {
                        handle_incoming_message(logger.new(o!("handling" => "incoming")), m, stats, &addr).await?
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
            CoordinatorResult::Broadcast(c) => {
                let m = c.into_message()?;
                let _ = tx.send(m).await;
            },
            CoordinatorResult::Outgoing(m) => {
                outgoing.send(m).await?;
            }
        }
    }
    info!(logger, "Client disconnected");
    Ok(())
}

async fn stats_collector_task(logger: Logger, stats: StatsCollector, mut rx: mpsc::Receiver<(SocketAddr, messages::Status)>) {
    debug!(logger, "Starting stats collector");

    while let Some((sock, status)) = rx.recv().await {
        debug!(logger, "Received stats for {} => {:?}", &sock, &status);
        stats.insert(sock, status);
    }
}

pub async fn run_forever(log: Logger, addr: String, web_addr: String) -> Result<()> {
    //let mut rt = runtime::Builder::new().threaded_scheduler().build()?;
    let (_s_tx, s_rx) = oneshot::channel();
    let (b_tx, b_rx) = watch::channel(messages::Command::Stop);
    let (stats_tx, stats_rx) = mpsc::channel(100);
    let stats = StatsCollector::new();
    tokio::spawn(webserver::webserver_task(log.clone(), web_addr, stats.clone(), b_tx));
    tokio::spawn(stats_collector_task(log.new(o!("task" => "stats")), stats.clone(), stats_rx));
    let res = tokio::spawn(start(
        log.new(o!("task" => "websocket")),
        addr,
        b_rx,
        s_rx,
        stats_tx,
    )).await;
    warn!(log, "Exiting");
    res?
}
