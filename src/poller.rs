use crate::stats::ConnectionStats;
use std::sync::mpsc;

#[derive(Debug, Clone, clap::Parser)]
pub struct StatsPollingOptions {
    pub sora_url: String,

    #[clap(long, default_value_t = 5.0)]
    pub polling_interval: f64,
}

impl StatsPollingOptions {
    pub fn start_polling_thread(&self) -> StatsReceiver {
        let (tx, rx) = mpsc::channel();
        let poller = StatsPoller {
            opt: self.clone(),
            tx,
        };
        std::thread::spawn(move || poller.run());
        rx
    }
}

pub type StatsReceiver = mpsc::Receiver<Vec<ConnectionStats>>;

type StatsSender = mpsc::Sender<Vec<ConnectionStats>>;

#[derive(Debug)]
struct StatsPoller {
    opt: StatsPollingOptions,
    tx: StatsSender,
}

impl StatsPoller {
    pub fn run(self) {
        todo!()
    }
}
