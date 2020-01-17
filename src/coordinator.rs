use slog::{
    Logger,
    debug,
    info,
    trace,
    o,
};
use anyhow::{Result, Error};
use async_tungstenite;
use tungstenite::protocol::Message;
use std::{
    sync::{Arc, Mutex},
    net::SocketAddr,
    io::Error as IoError,
    collections::HashMap,
    time::Duration,
};
use crate::messages;

use serde_json;

use futures::{
    select,
    channel::mpsc::{unbounded, UnboundedSender, TrySendError},
    stream::{self as futures_stream, TryStreamExt, StreamExt},
    Sink,
    future::{self, Either, Future, FutureExt},
    pin_mut,
};

use async_std::{
    net::{TcpListener, TcpStream},
    task,
    sync::{channel, Sender, Receiver},
    stream,
};

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

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
            //peer_map: Arc::new(Mutex::new(HashMap::new())),
            heartbeat,
            broadcast,
            stats,
        }
    }

    // fn insert_peer(&self, addr: SocketAddr, tx: Tx) {
    //     self.peer_map.lock().unwrap().insert(addr, tx);
    // }

    async fn recv_heartbeat(&self, tx: UnboundedSender<Message>) -> Result<()> {
        self.heartbeat.recv().await;
        tx.unbounded_send(Message::Ping(Vec::new()))?;
        Ok(())
    }

    async fn send_stats(&self, addr: &SocketAddr, stats: messages::Status) {
        self.stats.send((addr.clone(), stats)).await;
    }

    async fn recv_broadcast(&self, tx: UnboundedSender<Message>) -> Result<()> {
        let cmd = self.broadcast.recv().await;
        let s = serde_json::to_string(&cmd)?;
        tx.unbounded_send(Message::Text(s))?;
        Ok(())
    }
}

pub async fn heartbeat_task(logger: Logger, sender: Sender<()>, shutdown: Receiver<()>, period: Duration) {
    let stop = shutdown.recv().fuse();
    pin_mut!(stop);
    loop {
        let beat = task::sleep(period).fuse();
        pin_mut!(beat);
        let res = select! {
            b = beat => {
                debug!(logger, "Heartbeat triggered");
                sender.send(())
            },
            s = stop => break,
        };
        res.await
    }
    debug!(logger, "Heartbeat stopping");
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

async fn handle_incoming_message(log: Logger, msg: Message, state: State, addr: &SocketAddr, tx: UnboundedSender<Message>) -> Result<()> {
    //let status: messages::Status = serde_json::from_str(msg.to_text()?)?;
    match msg {
        Message::Ping(_) => {
            debug!(log, "Received ping");
            tx.unbounded_send(Message::Pong(Vec::new()))?;
        },
        Message::Pong(_) => {
            debug!(log, "Received pong");
        },
        Message::Text(t) => {
            debug!(log, "Received text");
            trace!(log, "Msg => {}", &t);
            let m: messages::Status = serde_json::from_str(&t)?;
            state.send_stats(addr, m).await;
        },
        Message::Close(_) => {
            debug!(log, "Received close");
        },
        _ => unimplemented!()
    }
    Ok(())
}

async fn handle_connection(logger: Logger, state: State, raw_stream: TcpStream, addr: SocketAddr) -> Result<()> {
    debug!(logger, "Client connected");
    let ws_stream = async_tungstenite::accept_async(raw_stream).await?;
    info!(logger, "WebSocket connection established");
    let (tx, rx) = unbounded();
    //state.insert_peer(addr, tx);
    let (outgoing, incoming) = ws_stream.split();
    let handle_incoming = incoming.map_err(|e| e.into()).try_for_each(|msg| handle_incoming_message(logger.new(o!("handling" => "incoming")), msg, state.clone(), &addr, tx.clone()));
    let handle_broadcast = state.recv_broadcast(tx.clone());
    let handle_heartbeat = state.recv_heartbeat(tx.clone());
    let responder = rx.map(Ok).forward(outgoing);
    select! {
        incoming_res = handle_incoming.fuse() => (),
        broadcast_res = handle_broadcast.fuse() => (),
        heartbeat_res = handle_heartbeat.fuse() => (),
        responder_res = responder.fuse() => (),
    };
    info!(logger, "Client disconnected");
    Ok(())
}

pub fn run_forever(log: Logger, addr: String) -> Result<()> {
    let (_s_tx, s_rx) = channel(1);
    let (_b_tx, b_rx) = channel(100);
    let (stats_tx, _stats_rx) = channel(100);
    task::block_on(start(
        log,
        addr,
        b_rx,
        s_rx,
        stats_tx,
    ))
}
