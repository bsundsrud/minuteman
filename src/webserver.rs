use std::{
    net::SocketAddr,
    collections::HashMap,
};
use crate::{
    messages,
    stats::StatsCollector
};
use slog::{Logger, info, debug, o};
use tide::{Request, Response, Server};
use anyhow::Result as TaskResult;

use serde::{Deserialize, Serialize};


struct State {
    stats: StatsCollector,
    logger: Logger,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatsResponse {
    items: HashMap<SocketAddr, messages::Status>
}

async fn get_stats(req: Request<State>) -> Response {
    match req.state().stats.serialize_all() {
        Ok(r) => Response::new(200).body_string(r),
        Err(e) => Response::new(500).body_string(format!("{}", e))
    }

}

pub async fn webserver_task(logger: Logger, addr: String, stats: StatsCollector) -> TaskResult<()> {
    let mut app = Server::with_state(State {
        stats,
        logger: logger.new(o!("task" => "webserver"))
    });

    app.at("/stats").get(get_stats);
    info!(logger, "Starting webserver at {}", addr);
    app.listen(addr).await?;
    Ok(())
}
