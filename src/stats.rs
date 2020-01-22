use crate::messages;
use anyhow::Result;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

struct StatsInner {
    started: Option<Instant>,
    elapsed: Option<Duration>,
    state: messages::WorkerState,
}
#[derive(Debug, Default)]
struct Counters {
    count: AtomicU32,
    count_1xx: AtomicU32,
    count_2xx: AtomicU32,
    count_3xx: AtomicU32,
    count_4xx: AtomicU32,
    count_5xx: AtomicU32,
}

impl Counters {
    fn new() -> Counters {
        Counters::default()
    }

    pub fn inc_count(&self) {
        self.count.fetch_add(1, Ordering::AcqRel);
    }

    pub fn inc_1xx(&self) {
        self.count_1xx.fetch_add(1, Ordering::AcqRel);
    }

    pub fn inc_2xx(&self) {
        self.count_2xx.fetch_add(1, Ordering::AcqRel);
    }

    pub fn inc_3xx(&self) {
        self.count_3xx.fetch_add(1, Ordering::AcqRel);
    }

    pub fn inc_4xx(&self) {
        self.count_4xx.fetch_add(1, Ordering::AcqRel);
    }

    pub fn inc_5xx(&self) {
        self.count_5xx.fetch_add(1, Ordering::AcqRel);
    }

    pub fn clear(&self) {
        self.count_1xx.store(0, Ordering::SeqCst);
        self.count_2xx.store(0, Ordering::SeqCst);
        self.count_3xx.store(0, Ordering::SeqCst);
        self.count_4xx.store(0, Ordering::SeqCst);
        self.count_5xx.store(0, Ordering::SeqCst);
    }
}

impl StatsInner {
    fn new() -> StatsInner {
        StatsInner {
            started: None,
            elapsed: None,
            state: messages::WorkerState::Idle,
        }
    }
}

#[derive(Clone)]
pub struct Stats(Arc<RwLock<StatsInner>>, Arc<Counters>);

impl Stats {
    pub fn new() -> Stats {
        Stats(
            Arc::new(RwLock::new(StatsInner::new())),
            Arc::new(Counters::new()),
        )
    }

    pub fn start(&self) {
        self.reset();
        let inner = self.0.clone();
        let mut stats = inner.write().unwrap();
        stats.state = messages::WorkerState::Busy;
        stats.started = Some(Instant::now());
    }

    pub fn stop(&self) {
        let inner = self.0.clone();
        let mut stats = inner.write().unwrap();
        stats.elapsed = stats.started.map(|s| s.elapsed());
        stats.state = messages::WorkerState::Idle;
    }

    pub fn reset(&self) {
        let inner = self.0.clone();
        let mut stats = inner.write().unwrap();
        let counters = self.1.clone();
        stats.elapsed = None;
        stats.started = None;
        stats.state = messages::WorkerState::Idle;
        counters.clear();
    }

    pub fn inc_count(&self) {
        let inner = self.1.clone();
        inner.inc_count();
    }

    pub fn record_status(&self, status: u16) {
        let counters = self.1.clone();
        if status >= 500 {
            counters.inc_5xx();
        } else if status >= 400 {
            counters.inc_4xx();
        } else if status >= 300 {
            counters.inc_3xx();
        } else if status >= 200 {
            counters.inc_2xx();
        } else if status >= 100 {
            counters.inc_1xx();
        }
    }

    pub fn into_message(&self) -> messages::Status {
        let inner = self.0.clone();
        let stats = inner.read().unwrap();
        let counters = self.1.clone();
        messages::Status {
            state: stats.state.clone(),
            elapsed: stats.elapsed.or(stats.started.map(|s| s.elapsed())),
            count: counters.count.load(Ordering::Acquire),
            count_1xx: counters.count_1xx.load(Ordering::Acquire),
            count_2xx: counters.count_2xx.load(Ordering::Acquire),
            count_3xx: counters.count_3xx.load(Ordering::Acquire),
            count_4xx: counters.count_4xx.load(Ordering::Acquire),
            count_5xx: counters.count_5xx.load(Ordering::Acquire),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatsCollector {
    stats: Arc<RwLock<HashMap<SocketAddr, messages::Status>>>,
}

impl StatsCollector {
    pub fn new() -> StatsCollector {
        StatsCollector {
            stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn insert(&self, socket: SocketAddr, stats: messages::Status) {
        let rc = self.stats.clone();
        let mut map = rc.write().unwrap();
        map.insert(socket, stats);
    }

    pub fn serialize_all(&self) -> Result<String> {
        let rc = self.stats.clone();
        let map: &HashMap<SocketAddr, messages::Status> = &rc.read().unwrap();
        Ok(serde_json::to_string(map)?)
    }
}
