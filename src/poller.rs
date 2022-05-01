use crate::stats::{ConnectionStats, ConnectionStats2, Stats2};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SORA_API_HEADER_NAME: &'static str = "x-sora-target";
const SORA_API_HEADER_VALUE: &'static str = "Sora_20171101.GetStatsAllConnections";

#[derive(Debug, Clone, clap::Parser)]
pub struct StatsPollingOptions {
    pub sora_api_url: String,

    #[clap(long, default_value_t = 1.0)]
    pub interval: f64, // TODO: NonZeroUsize

    #[clap(long, short, default_value = ".*:.*")]
    pub connection_filter: regex::Regex,

    #[clap(long, short = 'k', default_value = ".*")]
    pub stats_key_filter: regex::Regex,
}

impl StatsPollingOptions {
    fn polling_interval(&self) -> Duration {
        Duration::from_secs_f64(self.interval)
    }

    fn apply_filter(&self, connections: Vec<ConnectionStats>) -> Vec<ConnectionStats> {
        connections
            .into_iter()
            .filter(|c| {
                c.stats
                    .iter()
                    .any(|(k, v)| self.connection_filter.is_match(&format!("{}:{}", k, v)))
            })
            .map(|mut c| {
                let stats = c
                    .stats
                    .into_iter()
                    .filter(|(k, _v)| self.stats_key_filter.is_match(k))
                    .collect();
                c.stats = stats;
                c
            })
            .collect()
    }

    fn apply_filter2(&self, connections: Vec<ConnectionStats2>) -> Vec<ConnectionStats2> {
        connections
            .into_iter()
            .filter(|c| {
                c.stats.iter().any(|(k, v)| {
                    self.connection_filter
                        .is_match(&format!("{}:{}", k, v.value))
                })
            })
            .map(|mut c| {
                let stats = c
                    .stats
                    .into_iter()
                    .filter(|(k, _v)| self.stats_key_filter.is_match(k))
                    .collect();
                c.stats = stats;
                c
            })
            .collect()
    }
}

impl StatsPollingOptions {
    pub fn start_polling_thread(&self) -> anyhow::Result<StatsReceiver> {
        let (tx, rx) = mpsc::channel();
        let mut poller = StatsPoller {
            opt: self.clone(),
            tx,
            last_request_time: Instant::now(),
            prev: Stats2::empty(),
        };
        poller.poll_once()?;
        std::thread::spawn(move || poller.run());
        Ok(rx)
    }
}

pub type StatsReceiver = mpsc::Receiver<Vec<ConnectionStats>>;

type StatsSender = mpsc::Sender<Vec<ConnectionStats>>;

#[derive(Debug)]
struct StatsPoller {
    opt: StatsPollingOptions,
    tx: StatsSender,
    last_request_time: Instant,
    prev: Stats2,
}

impl StatsPoller {
    pub fn run(mut self) {
        loop {
            match self.run_once() {
                Err(e) => {
                    log::error!("failed to poll Sora stats: {}", e);
                    break;
                }
                Ok(false) => {
                    log::debug!("stop polling as the main thread has finished");
                    break;
                }
                Ok(true) => {}
            }
        }
    }

    fn run_once(&mut self) -> anyhow::Result<bool> {
        if let Some(duration) = self
            .opt
            .polling_interval()
            .checked_sub(self.last_request_time.elapsed())
        {
            std::thread::sleep(duration);
        }
        self.poll_once()
    }

    fn poll_once(&mut self) -> anyhow::Result<bool> {
        self.last_request_time = Instant::now();
        let values: Vec<serde_json::Value> = ureq::post(&self.opt.sora_api_url)
            .set(SORA_API_HEADER_NAME, SORA_API_HEADER_VALUE)
            .call()?
            .into_json()?;
        log::debug!(
            "HTTP POST {} {}:{} (elapsed: {:?}, connections: {})",
            self.opt.sora_api_url,
            SORA_API_HEADER_NAME,
            SORA_API_HEADER_VALUE,
            self.last_request_time.elapsed(),
            values.len()
        );

        let mut connections = Vec::new();
        let mut connections2 = Vec::new();
        for value in values {
            connections.push(ConnectionStats::from_json(value.clone())?);
            connections2.push(ConnectionStats2::new(value, &self.prev)?);
        }
        let connections = self.opt.apply_filter(connections);
        let connections2 = self.opt.apply_filter2(connections2);
        self.prev = Stats2::new(connections2);
        Ok(self.tx.send(connections).is_ok())
    }
}
