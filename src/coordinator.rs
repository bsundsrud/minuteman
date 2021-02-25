use crate::{messages, stats::StatsCollector, webserver};
use anyhow::Result;
use async_tungstenite::{self, tokio::TokioAdapter};
use futures::{
    pin_mut,
    sink::SinkExt,
    stream::{self, StreamExt, TryStreamExt},
};
use slog::{debug, info, o, warn, Logger};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    self,
    net::{TcpListener, TcpStream},
    runtime,
    sync::{mpsc, oneshot, watch},
    time,
};
use tokio_stream::wrappers::{ReceiverStream, WatchStream};
use tungstenite::protocol::Message;

#[derive(Clone, Debug)]
struct State {
    //peer_map: PeerMap,
    heartbeat: watch::Receiver<()>,
    broadcast: watch::Receiver<messages::Command>,
    stats: mpsc::Sender<(u32, messages::Status)>,
    collector: StatsCollector,
}

impl State {
    fn new(
        heartbeat: watch::Receiver<()>,
        broadcast: watch::Receiver<messages::Command>,
        stats: mpsc::Sender<(u32, messages::Status)>,
        collector: StatsCollector,
    ) -> State {
        State {
            heartbeat,
            broadcast,
            stats,
            collector,
        }
    }
}

pub async fn heartbeat_task(
    logger: Logger,
    sender: watch::Sender<()>,
    shutdown: oneshot::Receiver<()>,
    period: Duration,
) {
    debug!(logger, "Heartbeat starting");
    let timeout = time::interval(period);
    pin_mut!(shutdown, timeout);
    loop {
        timeout.tick().await;
        if shutdown.try_recv().is_ok() {
            debug!(logger, "Heartbeat task exiting");
            return;
        }
        debug!(logger, "Sending heartbeat");
        if let Err(e) = sender.send(()) {
            info!(logger, "Heartbeat channel error: {}", e);
            return;
        }
    }
}

pub async fn start(
    logger: Logger,
    addr: String,
    broadcast: watch::Receiver<messages::Command>,
    shutdown: oneshot::Receiver<()>,
    stats: mpsc::Sender<(u32, messages::Status)>,
    collector: StatsCollector,
) -> Result<()> {
    debug!(logger, "Starting coordinator");
    let (hb_tx, hb_rx) = watch::channel(());
    tokio::spawn(heartbeat_task(
        logger.new(o!("task" => "heartbeat")),
        hb_tx,
        shutdown,
        Duration::from_secs(5),
    ));

    let state = State::new(hb_rx, broadcast, stats, collector);
    let listener = TcpListener::bind(&addr).await?;
    info!(logger, "Listening on {}", &addr);
    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(handle_connection(
            logger.new(o!("client" => addr)),
            state.clone(),
            stream,
            addr,
        ));
    }
}

async fn handle_incoming_message(
    log: Logger,
    msg: Message,
    stats: mpsc::Sender<(u32, messages::Status)>,
    addr: &SocketAddr,
    id: u32,
) -> Result<bool> {
    let mut exit = false;
    match msg {
        Message::Ping(_) => {
            debug!(log, "Received ping");
        }
        Message::Pong(_) => {
            debug!(log, "Received pong");
        }
        Message::Text(t) => {
            let mut m: messages::Status = serde_json::from_str(&t)?;
            m.socket = Some(*addr);
            debug!(log, "Received Status => {:?}", m);
            let _ = stats.send((id, m)).await;
        }
        Message::Close(_) => {
            exit = true;
            debug!(log, "Received close");
        }
        _ => unimplemented!(),
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

async fn handle_connection(
    logger: Logger,
    state: State,
    raw_stream: TcpStream,
    addr: SocketAddr,
) -> Result<()> {
    debug!(logger, "Client connected");
    let ws_stream = async_tungstenite::accept_async(TokioAdapter::new(raw_stream)).await?;
    let id = state.collector.connect(addr);
    info!(logger, "WebSocket connection established");
    let (tx, rx) = mpsc::channel(100);

    let (mut outgoing, incoming) = ws_stream.split();
    let handle_incoming = incoming
        .map_err(|e| e.into())
        .map(CoordinatorResult::Incoming);

    let handle_broadcast =
        WatchStream::new(state.broadcast.clone()).map(CoordinatorResult::Broadcast);
    let handle_heartbeat =
        WatchStream::new(state.heartbeat.clone()).map(|_| CoordinatorResult::Heartbeat);

    let responder = ReceiverStream::new(rx).map(CoordinatorResult::Outgoing);

    let s1 = stream::select(handle_heartbeat, handle_broadcast);
    let s2 = stream::select(handle_incoming, responder);
    let mut combined = stream::select(s1, s2);
    while let Some(r) = combined.next().await {
        debug!(logger, "Action: {:?}", r);
        match r {
            CoordinatorResult::Heartbeat => {
                let _ = tx.send(Message::Ping(Vec::new())).await;
            }
            CoordinatorResult::Incoming(m) => {
                let stats = state.stats.clone();
                let exit = match m {
                    Ok(m) => {
                        handle_incoming_message(
                            logger.new(o!("handling" => "incoming")),
                            m,
                            stats,
                            &addr,
                            id,
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
            CoordinatorResult::Broadcast(c) => {
                let m = c.as_message()?;
                let _ = tx.send(m).await;
            }
            CoordinatorResult::Outgoing(m) => {
                outgoing.send(m).await?;
            }
        }
    }
    info!(logger, "Client disconnected");
    state.collector.disconnect(id);
    Ok(())
}

async fn stats_collector_task(
    logger: Logger,
    stats: StatsCollector,
    mut rx: mpsc::Receiver<(u32, messages::Status)>,
) {
    debug!(logger, "Starting stats collector");

    while let Some((id, status)) = rx.recv().await {
        debug!(logger, "Received stats => {:?}", &status);
        if let Err(e) = stats.insert(id, status) {
            warn!(logger, "Error inserting stats: {}", e);
        }
    }
}

pub fn run_forever(log: Logger, addr: String, web_addr: String) -> Result<()> {
    let rt = runtime::Builder::new_multi_thread().enable_all().build()?;
    let res = rt.block_on(async {
        let (_s_tx, s_rx) = oneshot::channel();
        let (b_tx, b_rx) = watch::channel(messages::Command::Stop);
        let (stats_tx, stats_rx) = mpsc::channel(100);
        let stats = StatsCollector::new();
        tokio::spawn(webserver::webserver_task(
            log.clone(),
            web_addr,
            stats.clone(),
            b_tx,
        ));
        tokio::spawn(stats_collector_task(
            log.new(o!("task" => "stats")),
            stats.clone(),
            stats_rx,
        ));
        tokio::spawn(start(
            log.new(o!("task" => "websocket")),
            addr,
            b_rx,
            s_rx,
            stats_tx,
            stats,
        ))
        .await
    });
    warn!(log, "Exiting");
    res?
}
