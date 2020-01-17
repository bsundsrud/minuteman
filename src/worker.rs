use slog::{
    Logger,
    info,
    debug,
    trace,
    o,
};
use anyhow::Result;
use async_std::task;
use async_tungstenite::async_std::connect_async;
use async_std::sync::{Sender, Receiver, channel};
use futures::{future, pin_mut, StreamExt, TryStreamExt};
use tungstenite::protocol::Message;
use serde_json;
use crate::messages;

#[derive(Debug, Clone)]
struct State {
    commands: Sender<messages::Command>,
}

impl State {
    fn new(commands: Sender<messages::Command>) -> State {
        State {
            commands,
        }
    }
}

async fn handle_message(logger: Logger, msg: Message, tx: Sender<Message>, cmd: Sender<messages::Command>) -> Result<()> {
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
        },
        _ => unimplemented!()
    }
    Ok(())
}
async fn run(logger: Logger, addr: String, state: State) -> Result<()> {
    info!(logger, "Connecting to {}", addr);
    let url = url::Url::parse(&addr)?;
    let (ws_stream, _response) = connect_async(url).await?;
    debug!(logger, "Successfully connected");
    let (tx, rx) = channel(100);
    let (outgoing, incoming) = ws_stream.split();
    let l = logger.new(o!("handling" => "incoming"));
    let handle_incoming = incoming
        .map_err(|e| e.into())
        .inspect(|m| debug!(l, "Receive Message => {:?}", m))
        .try_for_each(|msg| handle_message(logger.new(o!("handling" => "incoming")), msg, tx.clone(), state.commands.clone()));
    let l = logger.new(o!("handling" => "outgoing"));
    let handle_outgoing = rx
        .inspect(|m| debug!(l, "Sending message => {:?}", m))
        .map(Ok)
        .forward(outgoing);
    pin_mut!(handle_incoming, handle_outgoing);
    future::select(handle_incoming, handle_outgoing).await;
    Ok(())
}

async fn command_executor(logger: Logger, rx: Receiver<messages::Command>) -> Result<()> {
    debug!(logger, "Started executor task");
    while let Some(cmd) = rx.recv().await {
        info!(logger, "Received command {:?}", cmd);
    }
    Ok(())
}

pub fn run_forever(logger: Logger, addr: String) -> Result<()> {
    let (c_tx, c_rx) = channel(100);
    let state = State::new(c_tx);
    task::spawn(command_executor(logger.new(o!("task" => "executor")), c_rx));
    let res = task::block_on(run(logger.new(o!("task" => "receiver")), addr, state));
    debug!(logger, "After block");
    res
}
