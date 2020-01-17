use slog::{
    Logger,
    info,
    debug,
    o,
};
use anyhow::Result;
use async_std::task;
use futures::channel::mpsc::unbounded;
use async_tungstenite::async_std::connect_async;

pub async fn start(logger: Logger, addr: String) -> Result<()> {
    info!(logger, "Connecting to {}", addr);
    let url = url::Url::parse(&addr)?;
    let (ws_stream, _response) = connect_async(url).await?;
    debug!(logger, "Successfully connected");


    unimplemented!()
}

pub fn run_forever(logger: Logger, addr: String) -> Result<()> {
    task::block_on(start(logger, addr))
}
