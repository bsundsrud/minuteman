use crate::messages;
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

struct StatsInner {
    started: Option<Instant>,
    state: messages::WorkerState,
    counters: Arc<Counters>,
}
#[derive(Debug, Default)]
struct Counters {
    count: AtomicU32,
    count_2xx: AtomicU32,
    count_3xx: AtomicU32,
    count_4xx: AtomicU32,
    count_5xx: AtomicU32,
}

impl Counters {
    fn new() -> Counters {
        Counters::default()
    }
}

impl StatsInner {
    fn new() -> StatsInner {
        StatsInner {
            started: None,
            state: messages::WorkerState::Idle,
            counters: Arc::new(Counters::new()),
        }
    }
}

pub struct Stats(Arc<RwLock<StatsInner>>);

impl Stats {
    pub fn new() -> Stats {
        Stats(Arc::new(RwLock::new(StatsInner::new())))
    }

    pub fn into_message(&self) -> messages::Status {
        let inner = self.0.clone();
        let stats = inner.read().unwrap();
        let counters = stats.counters.clone();
        messages::Status {
            state: stats.state.clone(),
            elapsed: stats.started.map(|s| s.elapsed()),
            count: counters.count.load(Ordering::Acquire),
            count_2xx: counters.count_2xx.load(Ordering::Acquire),
            count_3xx: counters.count_3xx.load(Ordering::Acquire),
            count_4xx: counters.count_4xx.load(Ordering::Acquire),
            count_5xx: counters.count_5xx.load(Ordering::Acquire),
        }
    }
}
