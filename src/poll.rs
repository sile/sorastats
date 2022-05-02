use crate::stats::{ConnectionStats, Stats};
use crate::Options;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SORA_API_HEADER_NAME: &str = "x-sora-target";
const SORA_API_HEADER_VALUE: &str = "Sora_20171101.GetStatsAllConnections";

pub type StatsReceiver = mpsc::Receiver<Stats>;
pub type StatsSender = mpsc::Sender<Stats>;

#[derive(Debug)]
pub struct StatsPoller {
    options: Options,
    tx: StatsSender,
    prev_request_time: Instant,
    prev_stats: Stats,
}

impl StatsPoller {
    pub fn start_thread(options: Options) -> anyhow::Result<StatsReceiver> {
        let (tx, rx) = mpsc::channel();
        let mut poller = StatsPoller {
            options,
            tx,
            prev_request_time: Instant::now(),
            prev_stats: Stats::empty(),
        };
        poller.poll_once()?;
        std::thread::spawn(move || poller.run());
        Ok(rx)
    }

    fn run(mut self) {
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
        let polling_interval = Duration::from_secs(self.options.polling_interval.get() as u64);
        if let Some(duration) = polling_interval.checked_sub(self.prev_request_time.elapsed()) {
            std::thread::sleep(duration);
        }
        self.poll_once()
    }

    fn poll_once(&mut self) -> anyhow::Result<bool> {
        self.prev_request_time = Instant::now();
        let values: Vec<serde_json::Value> = ureq::post(&self.options.sora_api_url)
            .set(SORA_API_HEADER_NAME, SORA_API_HEADER_VALUE)
            .call()?
            .into_json()?;
        log::debug!(
            "HTTP POST {} {}:{} (elapsed: {:?}, connections: {})",
            self.options.sora_api_url,
            SORA_API_HEADER_NAME,
            SORA_API_HEADER_VALUE,
            self.prev_request_time.elapsed(),
            values.len()
        );

        let mut connections = Vec::new();
        for value in values {
            connections.push(ConnectionStats::new(value, &self.prev_stats)?);
        }
        let connections = self.apply_filters(connections);
        self.prev_stats = Stats::new(connections);
        Ok(self.tx.send(self.prev_stats.clone()).is_ok())
    }

    fn apply_filters(&self, connections: Vec<ConnectionStats>) -> Vec<ConnectionStats> {
        connections
            .into_iter()
            .filter(|c| {
                c.items.iter().any(|(k, v)| {
                    self.options
                        .connection_filter
                        .is_match(&format!("{}:{}", k, v.value))
                })
            })
            .map(|mut c| {
                c.items = c
                    .items
                    .into_iter()
                    .filter(|(k, _v)| self.options.stats_key_filter.is_match(k))
                    .collect();
                c
            })
            .collect()
    }
}
