use crate::messages;
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Error, Result};
use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};

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
    count_fail: AtomicU32,
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

    pub fn inc_fail(&self) {
        self.count_fail.fetch_add(1, Ordering::AcqRel);
    }

    pub fn clear(&self) {
        self.count.store(0, Ordering::SeqCst);
        self.count_1xx.store(0, Ordering::SeqCst);
        self.count_2xx.store(0, Ordering::SeqCst);
        self.count_3xx.store(0, Ordering::SeqCst);
        self.count_4xx.store(0, Ordering::SeqCst);
        self.count_5xx.store(0, Ordering::SeqCst);
        self.count_fail.store(0, Ordering::SeqCst);
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
pub struct Stats {
    inner: Arc<RwLock<StatsInner>>,
    counters: Arc<Counters>,
    histo: Arc<RwLock<Histogram<u64>>>,
    task_gauge: Arc<AtomicU32>,
    task_queue: Arc<AtomicU32>,
    task_max: Arc<AtomicU32>,
}

impl Stats {
    pub fn new() -> Stats {
        let mut histo = Histogram::new_with_max(60_000, 5).unwrap();
        histo.auto(true);
        Stats {
            inner: Arc::new(RwLock::new(StatsInner::new())),
            counters: Arc::new(Counters::new()),
            histo: Arc::new(RwLock::new(histo)),
            task_gauge: Arc::new(AtomicU32::new(0)),
            task_queue: Arc::new(AtomicU32::new(0)),
            task_max: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn start(&self) {
        self.reset();
        let mut stats = self.inner.write().unwrap();
        stats.state = messages::WorkerState::Busy;
        stats.started = Some(Instant::now());
    }

    pub fn stop(&self) {
        let mut stats = self.inner.write().unwrap();
        stats.elapsed = stats.started.map(|s| s.elapsed());
        stats.state = messages::WorkerState::Idle;
    }

    pub fn reset(&self) {
        let mut stats = self.inner.write().unwrap();
        let counters = self.counters.clone();
        let mut histo = self.histo.write().unwrap();
        histo.reset();
        stats.elapsed = None;
        stats.started = None;
        stats.state = messages::WorkerState::Idle;
        counters.clear();
        self.task_gauge.store(0, Ordering::Release);
        self.task_queue.store(0, Ordering::Release);
        self.task_max.store(0, Ordering::Release);
    }

    pub fn record_task_max(&mut self, max: u32) {
        self.task_max.store(max, Ordering::Release);
    }

    pub fn record_current_tasks(&mut self, current: u32) {
        self.task_gauge.store(current, Ordering::Release);
    }

    pub fn record_queue_depth(&mut self, current: u32) {
        self.task_queue.store(current, Ordering::Release);
    }

    pub fn record(&mut self, status: Option<u16>, elapsed_ms: u64) {
        let counters = self.counters.clone();
        counters.inc_count();
        match status {
            Some(status) => {
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
            None => counters.inc_fail(),
        }
        let mut histo = self.histo.write().unwrap();
        histo.record(elapsed_ms).unwrap();
    }

    pub fn as_message(&self) -> messages::Status {
        let stats = self.inner.read().unwrap();
        let counters = self.counters.clone();
        let histo = self.histo.read().unwrap();
        let min = histo.min();
        let max = histo.max();
        let mean = histo.mean();
        let stdev = histo.stdev();
        let median = histo.value_at_quantile(0.5);
        let p90 = histo.value_at_quantile(0.9);
        drop(histo);
        messages::Status {
            hostname: None,
            socket: None,
            state: stats.state,
            elapsed: stats.elapsed.or_else(|| stats.started.map(|s| s.elapsed())),
            tasks: self.task_gauge.load(Ordering::Acquire),
            task_queue: self.task_queue.load(Ordering::Acquire),
            tasks_max: self.task_max.load(Ordering::Acquire),
            min,
            max,
            mean,
            stdev,
            median,
            p90,
            count: counters.count.load(Ordering::Acquire),
            count_1xx: counters.count_1xx.load(Ordering::Acquire),
            count_2xx: counters.count_2xx.load(Ordering::Acquire),
            count_3xx: counters.count_3xx.load(Ordering::Acquire),
            count_4xx: counters.count_4xx.load(Ordering::Acquire),
            count_5xx: counters.count_5xx.load(Ordering::Acquire),
            count_fail: counters.count_fail.load(Ordering::Acquire),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerState {
    Connected,
    Idle,
    Busy,
    Disconnected,
}

impl From<messages::WorkerState> for WorkerState {
    fn from(s: messages::WorkerState) -> WorkerState {
        match s {
            messages::WorkerState::Idle => WorkerState::Idle,
            messages::WorkerState::Busy => WorkerState::Busy,
        }
    }
}

#[derive(Debug)]
pub struct Snapshot {
    pub timestamp: SystemTime,
    pub state: WorkerState,
    pub elapsed: Option<Duration>,
    pub tasks: u32,
    pub task_queue: u32,
    pub tasks_max: u32,
    pub min: u64,
    pub max: u64,
    pub mean: f64,
    pub stdev: f64,
    pub median: u64,
    pub p90: u64,
    pub count: u32,
    pub count_1xx: u32,
    pub count_2xx: u32,
    pub count_3xx: u32,
    pub count_4xx: u32,
    pub count_5xx: u32,
    pub count_fail: u32,
}

impl From<messages::Status> for Snapshot {
    fn from(s: messages::Status) -> Snapshot {
        Snapshot {
            timestamp: SystemTime::now(),
            state: s.state.into(),
            elapsed: s.elapsed,
            tasks: s.tasks,
            task_queue: s.task_queue,
            tasks_max: s.tasks_max,
            min: s.min,
            max: s.max,
            mean: s.mean,
            stdev: s.stdev,
            median: s.median,
            p90: s.p90,
            count: s.count,
            count_1xx: s.count_1xx,
            count_2xx: s.count_2xx,
            count_3xx: s.count_3xx,
            count_4xx: s.count_4xx,
            count_5xx: s.count_5xx,
            count_fail: s.count_fail,
        }
    }
}

#[derive(Debug)]
pub struct Status {
    pub id: u32,
    pub hostname: Option<String>,
    pub socket: SocketAddr,
    pub state: WorkerState,
    pub connect_time: SystemTime,
    pub disconnect_time: Option<SystemTime>,
    pub snapshots: VecDeque<Snapshot>,
}

impl Status {
    pub fn connect(id: u32, socket: SocketAddr, hostname: Option<String>) -> Status {
        Status {
            id,
            hostname,
            socket,
            state: WorkerState::Connected,
            connect_time: SystemTime::now(),
            disconnect_time: None,
            snapshots: VecDeque::new(),
        }
    }

    pub fn record(&mut self, status: messages::Status) {
        self.state = status.state.into();
        self.hostname = status.hostname.clone();
        self.snapshots.push_front(status.into());
        self.snapshots.truncate(100);
    }

    pub fn disconnect(&mut self) {
        self.state = WorkerState::Disconnected;
        self.disconnect_time = Some(SystemTime::now());
    }

    pub fn is_disconnected(&self) -> bool {
        self.state == WorkerState::Disconnected
    }
}

#[derive(Debug, Clone)]
pub struct StatsCollector {
    stats: Arc<RwLock<HashMap<u32, Status>>>,
    id_counter: Arc<AtomicU32>,
}

impl StatsCollector {
    pub fn new() -> StatsCollector {
        StatsCollector {
            stats: Arc::new(RwLock::new(HashMap::new())),
            id_counter: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn connect(&self, socket: SocketAddr) -> u32 {
        let rc = self.stats.clone();
        let mut map = rc.write().unwrap();
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        map.insert(id, Status::connect(id, socket, None));
        id
    }

    pub fn insert(&self, id: u32, stats: messages::Status) -> Result<()> {
        let rc = self.stats.clone();
        let mut map = rc.write().unwrap();
        if let Some(s) = map.get_mut(&id) {
            s.record(stats);
        } else {
            return Err(Error::msg(format!(
                "ID {} not found in current clients",
                id
            )));
        }
        Ok(())
    }

    pub fn prune_disconnected(&self) {
        let rc = self.stats.clone();
        let mut map = rc.write().unwrap();
        map.retain(|_, v| !v.is_disconnected());
    }

    pub fn disconnect(&self, id: u32) {
        let rc = self.stats.clone();
        let mut map = rc.write().unwrap();
        if let Some(s) = map.get_mut(&id) {
            s.disconnect();
        }
    }

    pub fn with_stats<F, V>(&self, f: F) -> V
    where
        F: Fn(&HashMap<u32, Status>) -> V,
    {
        let rc = self.stats.clone();
        let rc = rc.read().unwrap();
        f(&rc)
    }
}
