use slog::{
    Logger,
    debug,
    info,
    trace,
    warn,
    o,
};
use anyhow::Result;
use async_tungstenite;
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
    stream::{self as futures_stream, TryStreamExt, StreamExt},
    sink::SinkExt,
    pin_mut,
};

use async_std::{
    net::{TcpListener, TcpStream},
    task,
    sync::{channel, Sender, Receiver},
    stream,
};

#[derive(Clone, Debug)]
struct State {
    //peer_map: PeerMap,
    heartbeat: Receiver<()>,
    broadcast: Receiver<messages::Command>,
    stats: Sender<(SocketAddr, messages::Status)>,
}

impl State {
    fn new(heartbeat: Receiver<()>, broadcast: Receiver<messages::Command>, stats: Sender<(SocketAddr, messages::Status)>) -> State {
        State {
            heartbeat,
            broadcast,
            stats,
        }
    }
}

pub async fn heartbeat_task(logger: Logger, sender: Sender<()>, shutdown: Receiver<()>, period: Duration) {
    debug!(logger, "Heartbeat starting");
    let timeout = stream::interval(period);
    pin_mut!(shutdown, timeout);
    while let Some(_) = timeout.next().await {
        if shutdown.len() > 0 {
            drop(shutdown);
            debug!(logger, "Heartbeat task exiting");
            return
        }
        if sender.len() == 0 {
            debug!(logger, "Sending heartbeat");
            sender.send(()).await;
        }
    }
}

pub async fn start(logger: Logger, addr: String, broadcast: Receiver<messages::Command>, shutdown: Receiver<()>, stats: Sender<(SocketAddr, messages::Status)>) -> Result<()> {
    debug!(logger, "Starting coordinator");
    let (hb_tx, hb_rx) = channel(1);
    task::spawn(heartbeat_task(logger.new(o!("task" => "heartbeat")), hb_tx, shutdown.clone(), Duration::from_secs(1)));

    let state = State::new(hb_rx, broadcast, stats);
    let listener = TcpListener::bind(&addr).await?;
    info!(logger, "Listening on {}", &addr);
    while let Ok((stream, addr)) = listener.accept().await {
        task::spawn(handle_connection(logger.new(o!("client" => addr)), state.clone(), stream, addr));
    }
    Ok(())
}

async fn handle_incoming_message(log: Logger, msg: Message, stats: Sender<(SocketAddr, messages::Status)>, addr: &SocketAddr, tx: Sender<Message>) -> Result<bool> {
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
            stats.send((addr.clone(), m)).await;
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
    let ws_stream = async_tungstenite::accept_async(raw_stream).await?;
    info!(logger, "WebSocket connection established");
    let (tx, rx) = channel(100);

    let (mut outgoing, incoming) = ws_stream.split();
    let handle_incoming = incoming
        .inspect(|m| debug!(logger.new(o!("handling" => "incoming")), "Received Message => {:?}", m))
        .map_err(|e| e.into())
        .map(|m| CoordinatorResult::Incoming(m));

    let handle_broadcast = state.broadcast
        .inspect(|c| debug!(logger.new(o!("action" => "broadcast")), "Sending cmd => {:?}", c))
        .map(|c| CoordinatorResult::Broadcast(c));
    let handle_heartbeat = state.heartbeat
        .inspect(|_| debug!(logger.new(o!("action" => "heartbeat")), "Sending ping"))
        .map(|_| CoordinatorResult::Heartbeat);

    let responder = rx.clone()
        .inspect(|m| debug!(logger.new(o!("action" => "send")), "Sending msg => {:?}", m))
        .map(|m| CoordinatorResult::Outgoing(m));

    let s1 = futures_stream::select(handle_heartbeat, handle_broadcast);
    let s2 = futures_stream::select(handle_incoming, responder);
    let mut combined = futures_stream::select(s1, s2);
    while let Some(r) = combined.next().await {
        debug!(logger, "Action: {:?}", r);
        match r {
            CoordinatorResult::Heartbeat => {
                tx.send(Message::Ping(Vec::new())).await;
            },
            CoordinatorResult::Incoming(m) => {
                let stats = state.stats.clone();
                let exit = match m {
                    Ok(m) => {
                        handle_incoming_message(logger.new(o!("handling" => "incoming")), m, stats, &addr, tx.clone()).await?
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
                tx.send(m).await;
            },
            CoordinatorResult::Outgoing(m) => {
                outgoing.send(m).await?;
            }
        }
    }
    info!(logger, "Client disconnected");
    Ok(())
}

async fn stats_collector_task(logger: Logger, stats: StatsCollector, rx: Receiver<(SocketAddr, messages::Status)>) {
    debug!(logger, "Starting stats collector");

    while let Some((sock, status)) = rx.recv().await {
        debug!(logger, "Received stats for {} => {:?}", &sock, &status);
        stats.insert(sock, status);
    }
}

pub fn run_forever(log: Logger, addr: String, web_addr: String) -> Result<()> {
    let (_s_tx, s_rx) = channel(1);
    let (_b_tx, b_rx) = channel(100);
    let (stats_tx, stats_rx) = channel(100);
    let stats = StatsCollector::new();
    task::spawn(webserver::webserver_task(log.clone(), web_addr, stats.clone()));
    task::spawn(stats_collector_task(log.new(o!("task" => "stats")), stats.clone(), stats_rx));
    task::block_on(start(
        log,
        addr,
        b_rx,
        s_rx,
        stats_tx,
    ))
}
