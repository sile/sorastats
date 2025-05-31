use crate::stats::{ConnectionStats, Stats};
use crate::Options;
use orfail::OrFail;
use std::fs::File;
use std::io::{BufRead as _, BufReader, BufWriter, Write as _};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

const SORA_API_HEADER_NAME: &str = "x-sora-target";
const SORA_API_HEADER_VALUE: &str = "Sora_20171101.GetStatsAllConnections";

pub type StatsReceiver = mpsc::Receiver<Option<Stats>>;

#[derive(Debug)]
enum Mode {
    Realtime {
        tx: mpsc::Sender<Option<Stats>>,
    },
    Replay {
        tx: mpsc::SyncSender<Option<Stats>>,
        reader: BufReader<File>,
    },
}

#[derive(Debug)]
pub struct StatsPoller {
    options: Options,
    mode: Mode,
    prev_request_time: Instant,
    prev_stats: Stats,
    recorder: Option<BufWriter<File>>,
    start: Option<SystemTime>,
}

impl StatsPoller {
    pub fn start_thread(options: Options) -> orfail::Result<StatsReceiver> {
        let recorder = options.create_recorder()?;

        let (rx, mode) = if options.is_realtime_mode() {
            let (tx, rx) = mpsc::channel();
            (rx, Mode::Realtime { tx })
        } else {
            let (tx, rx) = mpsc::sync_channel(0);
            let file = File::open(&options.sora_api_url).or_fail_with(|e| {
                format!("failed to open record file {:?}: {e}", options.sora_api_url)
            })?;
            let reader = BufReader::new(file);
            (rx, Mode::Replay { tx, reader })
        };

        let mut poller = StatsPoller {
            options,
            mode,
            prev_request_time: Instant::now(),
            prev_stats: Stats::empty(),
            recorder,
            start: None,
        };
        match &mut poller.mode {
            Mode::Realtime { .. } => {
                poller.poll_once().or_fail()?;
            }
            Mode::Replay { reader, .. } => {
                if reader.get_mut().metadata().or_fail()?.len() == 0 {
                    return Err(orfail::Failure::new("empty record file"));
                }
            }
        }
        std::thread::spawn(move || poller.run());
        Ok(rx)
    }

    fn run(mut self) {
        loop {
            match self.run_once().or_fail() {
                Err(_e) => {
                    break;
                }
                Ok(false) => {
                    break;
                }
                Ok(true) => {}
            }
        }
    }

    fn run_once(&mut self) -> orfail::Result<bool> {
        if matches!(self.mode, Mode::Realtime { .. }) {
            let polling_interval = Duration::from_secs(self.options.polling_interval.get() as u64);
            if let Some(duration) = polling_interval.checked_sub(self.prev_request_time.elapsed()) {
                std::thread::sleep(duration);
            }
        }
        self.poll_once().or_fail()
    }

    fn poll_once(&mut self) -> orfail::Result<bool> {
        self.prev_request_time = Instant::now();
        let item = match &mut self.mode {
            Mode::Realtime { tx, .. } => {
                let request = ureq::post(&self.options.sora_api_url)
                    .header(SORA_API_HEADER_NAME, SORA_API_HEADER_VALUE);
                let request_result = if self.options.global {
                    request.send_json(serde_json::json!({"local": false}))
                } else {
                    request.send_empty()
                };
                let values: Vec<serde_json::Value> = match request_result {
                    Err(_e) => {
                        return Ok(tx.send(None).is_ok());
                    }
                    Ok(response) => response.into_body().read_json().or_fail()?,
                };
                let item = RecordItem {
                    time: SystemTime::now(),
                    values,
                };
                if let Some(mut recorder) = self.recorder.as_mut() {
                    serde_json::to_writer(&mut recorder, &item).or_fail()?;
                    writeln!(recorder).or_fail()?;
                    recorder.flush().or_fail()?;
                }
                item
            }
            Mode::Replay { reader, .. } => {
                let mut buf = String::new();
                let size = reader.read_line(&mut buf).or_fail()?;
                if size == 0 {
                    return Ok(false); // EOF
                }
                let item: RecordItem = serde_json::from_str(&buf).or_fail()?;
                item
            }
        };

        let start = if let Some(start) = self.start {
            start
        } else {
            self.start = Some(item.time);
            item.time
        };

        let mut connections = Vec::new();
        for value in item.values {
            connections.push(ConnectionStats::new(value, &self.prev_stats)?);
        }
        let connections = self.apply_connection_filters(connections);
        let timestamp = item.time.duration_since(start).or_fail()?;
        self.prev_stats = Stats::new(item.time, timestamp, connections);

        match &self.mode {
            Mode::Realtime { tx } => Ok(tx.send(Some(self.prev_stats.clone())).is_ok()),
            Mode::Replay { tx, .. } => Ok(tx.send(Some(self.prev_stats.clone())).is_ok()),
        }
    }

    fn apply_connection_filters(&self, connections: Vec<ConnectionStats>) -> Vec<ConnectionStats> {
        connections
            .into_iter()
            .filter(|c| {
                c.items.iter().any(|(k, v)| {
                    self.options
                        .connection_filter
                        .is_match(&format!("{}:{}", k, v.value))
                })
            })
            .collect()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RecordItem {
    time: SystemTime,
    values: Vec<serde_json::Value>,
}
