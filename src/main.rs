use anyhow::Result;
use async_std::task;
use slog::{debug, error, info, o, trace, warn, Drain, Logger};
use slog_async;
use slog_term;
use std::env;

mod coordinator;
mod messages;
mod worker;

fn root_logger() -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    Logger::root(drain, o!())
}

fn main() -> Result<()> {
    let log = root_logger();
    debug!(log, "Logger initialized");
    let res = if let Some(addr) = env::args().nth(1) {
        task::block_on(worker::start(log.new(o!("type" => "worker")), addr))
    } else {
        let addr = "0.0.0.0:5556".to_string();
        coordinator::run_forever(log.new(o!("type" => "coordinator")), addr)
    };
    debug!(log, "Exiting main.");
    res
}
