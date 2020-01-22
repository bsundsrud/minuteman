use futures::task::Poll;
use slog::{
    Logger,
    info,
    debug,
    trace,
    o,
    warn,
};
use anyhow::{Result, Error};
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
use surf;
use rand::{self, seq::SliceRandom};

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

async fn command_executor(logger: Logger, stats: Stats, rx: Receiver<messages::Command>) -> Result<()> {
    debug!(logger, "Started executor task");
    let (shutdown_tx, shutdown_rx) = channel::<bool>(10);
    let mut handle = None;
    while let Some(cmd) = rx.recv().await {
        info!(logger, "Received command {:?}", cmd);
        match cmd {
            messages::Command::Start { urls, strategy, max_concurrency } => {
                let drainer = shutdown_rx.clone();
                while drainer.len() > 0 {
                    drainer.recv().await;
                }
                stats.start();
                let h = task::spawn(task_scheduler(logger.new(o!("task" => "scheduler")), urls, stats.clone(), strategy, max_concurrency, shutdown_rx.clone()));
                handle = Some(h);
            },
            messages::Command::Stop => {
                if shutdown_tx.is_empty() {
                    debug!(logger, "Sending stop message");
                    shutdown_tx.send(false).await;
                }
                stats.stop();
            },
            messages::Command::Reset => {
                if shutdown_tx.is_empty() {
                    debug!(logger, "Sending stop message");
                    shutdown_tx.send(false).await;
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

async fn task_scheduler(logger: Logger, urls: Vec<String>, stats: Stats, strategy: messages::AttackStrategy, max_concurrency: u32, shutdown: Receiver<bool>) -> Result<()> {
    if urls.is_empty() {
        return Ok(());
    }
    debug!(logger, "Task Scheduler starting");
    let (start_tx, start_rx) = channel::<bool>(max_concurrency as usize);
    let creator = stream::repeat(true);
    let mut chooser = StrategyChooser::new();
    let mut combined = stream::select(creator, shutdown);
    let mut id: u64 = 0;
    while let Some(keep_going) = combined.next().await {
        if keep_going {
            start_tx.send(true).await;
            let url = chooser.choose(strategy, &urls).ok_or_else(|| Error::msg("Couldn't choose url".to_string()))?;
            let t = worker_task(logger.new(o!("task" => "worker")), url.into(), stats.clone(), id, start_rx.clone());
            id += 1;
            task::spawn(t);
        } else {
            debug!(logger, "Received stop message");
            break;
        }
    }
    debug!(logger, "Task Scheduler shutting down");
    Ok(())
}

async fn worker_task(logger: Logger, url: String, stats: Stats, id: u64, done: Receiver<bool>) -> Result<u64> {
    let res = surf::get(&url).await.map_err(|e| Error::msg(format!("{}", e)))?;
    let s = res.status().as_u16();
    stats.record_status(s);
    debug!(logger, "Worker {} result: {}", id, s);
    done.recv().await;
    Ok(id)
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
    task::spawn(stats_executor(logger.new(o!("task" => "stats")), stats.clone(), stats_tx));
    task::spawn(command_executor(logger.new(o!("task" => "executor")), stats.clone(), c_rx));
    let res = task::block_on(run(logger.new(o!("task" => "receiver")), addr, state));
    res
}
