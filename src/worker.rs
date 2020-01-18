use slog::{
    Logger,
    info,
    debug,
    trace,
    o,
    warn,
};
use anyhow::Result;
use async_std::{
    self,
    task,
    sync::{
        Sender,
        Receiver,
        channel},
};
use async_tungstenite::async_std::connect_async;
use futures::{
    future,
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
        Instant,
        Duration,
    }
};

use crate::stats::Stats;
use crate::messages;

#[derive(Debug, Clone)]
struct State {
    commands: Sender<messages::Command>,
    stats: Receiver<messages::Status>,
}

impl State {
    fn new(commands: Sender<messages::Command>, stats: Receiver<messages::Status>) -> State {
        State {
            commands,
            stats,
        }
    }
}

async fn handle_message(logger: Logger, msg: Message, tx: Sender<Message>, cmd: Sender<messages::Command>) -> Result<bool> {
    let mut exit = false;
    match msg {
        Message::Ping(_) => {
            debug!(logger, "Received Ping");
        },
        Message::Pong(_) => {
            debug!(logger, "Received Pong");
        },
        Message::Text(t) => {
            debug!(logger, "Received Text");
            trace!(logger, "msg => {}", t);
            let m: messages::Command = serde_json::from_str(&t)?;
            cmd.send(m).await;
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
    let (ws_stream, _response) = connect_async(url).await?;
    debug!(logger, "Successfully connected");
    let (tx, rx) = channel(100);
    let (mut outgoing, incoming) = ws_stream.split();
    let l = logger.new(o!("handling" => "incoming"));
    let handle_incoming = incoming
        .map_err(|e| e.into())
        .inspect(|m| debug!(l, "Receive Message => {:?}", m))
        .map(Action::Incoming);
        //.try_for_each(|msg| handle_message(logger.new(o!("handling" => "incoming")), msg, tx.clone(), state.commands.clone()));
    let l = logger.new(o!("handling" => "outgoing"));
    let handle_outgoing = rx
        .inspect(|m| debug!(l, "Sending message => {:?}", m))
        .map(Action::Outgoing);
    let handle_stats = state
        .stats
        .inspect(|m| debug!(logger.new(o!("action" => "stats")), "Sending stats => {:?}", m))
        .map(Action::Stats);
    let s1 = stream::select(handle_stats, handle_incoming);
    let mut combined = stream::select(s1, handle_outgoing);
    while let Some(r) = combined.next().await {
        debug!(logger, "Action: {:?}", r);
        match r {
            Action::Stats(s) => {
                tx.send(s.into_message()?).await;
            },
            Action::Incoming(m) => {
                let exit = match m {
                    Ok(m) => {
                        handle_message(logger.new(o!("handling" => "incoming")), m, tx.clone(), state.commands.clone()).await?
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
    Ok(())
}

async fn command_executor(logger: Logger, rx: Receiver<messages::Command>) -> Result<()> {
    debug!(logger, "Started executor task");
    while let Some(cmd) = rx.recv().await {
        info!(logger, "Received command {:?}", cmd);
    }
    Ok(())
}

async fn stats_executor(logger: Logger, stats: Stats, tx: Sender<messages::Status>) {
    debug!(logger, "Stats heartbeat starting");
    let timeout = async_std::stream::interval(Duration::from_secs(5));
    pin_mut!(timeout);
    while let Some(_) = timeout.next().await {
        if tx.len() == 0 {
            let s = stats.into_message();
            debug!(logger, "Sending stats => {:?}", &s);
            tx.send(s).await;
        }
    }
}

pub fn run_forever(logger: Logger, addr: String) -> Result<()> {
    let (c_tx, c_rx) = channel(100);
    let (stats_tx, stats_rx) = channel(1);
    let state = State::new(c_tx, stats_rx);
    let stats = Stats::new();
    task::spawn(stats_executor(logger.new(o!("task" => "stats")), stats, stats_tx));
    task::spawn(command_executor(logger.new(o!("task" => "executor")), c_rx));
    let res = task::block_on(run(logger.new(o!("task" => "receiver")), addr, state));
    res
}
