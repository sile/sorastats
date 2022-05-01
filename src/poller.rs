use crate::stats::ConnectionStats;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SORA_API_HEADER_NAME: &'static str = "x-sora-target";
const SORA_API_HEADER_VALUE: &'static str = "Sora_20171101.GetStatsAllConnections";

#[derive(Debug, Clone, clap::Parser)]
pub struct StatsPollingOptions {
    pub sora_url: String,

    #[clap(long, default_value_t = 1.0)]
    pub polling_interval: f64,
}

impl StatsPollingOptions {
    fn polling_interval(&self) -> Duration {
        Duration::from_secs_f64(self.polling_interval)
    }
}

impl StatsPollingOptions {
    pub fn start_polling_thread(&self) -> anyhow::Result<StatsReceiver> {
        let (tx, rx) = mpsc::channel();
        let mut poller = StatsPoller {
            opt: self.clone(),
            tx,
            last_request_time: Instant::now(),
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
        let values: Vec<serde_json::Value> = ureq::post(&self.opt.sora_url)
            .set(SORA_API_HEADER_NAME, SORA_API_HEADER_VALUE)
            .call()?
            .into_json()?;
        log::debug!(
            "HTTP POST {} {}:{} (elapsed: {:?}, connections: {})",
            self.opt.sora_url,
            SORA_API_HEADER_NAME,
            SORA_API_HEADER_VALUE,
            self.last_request_time.elapsed(),
            values.len()
        );

        let mut connections = Vec::new();
        for value in values {
            connections.push(ConnectionStats::from_json(value)?);
        }
        Ok(self.tx.send(connections).is_ok())
    }
}
