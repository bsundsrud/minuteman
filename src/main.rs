use anyhow::Result;
use async_std::task;
use slog::{debug, error, info, o, trace, warn, Drain, Logger};
use slog_async;
use slog_term;
use std::env;

mod coordinator;
mod messages;
mod worker;

fn root_logger() -> (Logger, slog_async::AsyncGuard) {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let (drain, guard) = slog_async::Async::new(drain).build_with_guard();
    let drain = drain.fuse();
    (Logger::root(drain, o!()), guard)
}

fn main() -> Result<()> {
    let (log, _guard) = root_logger();
    debug!(log, "Logger initialized");
    let res = if let Some(addr) = env::args().nth(1) {
        worker::run_forever(log.new(o!("type" => "worker")), addr)
    } else {
        let addr = "0.0.0.0:5556".to_string();
        coordinator::run_forever(log.new(o!("type" => "coordinator")), addr)
    };
    debug!(log, "Exiting main.");
    match &res {
        Ok(_) => {}
        Err(e) => error!(log, "{}", e),
    }
    res
}
