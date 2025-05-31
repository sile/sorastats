use orfail::OrFail;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

pub mod poll;
pub mod stats;
pub mod ui;

#[derive(Debug, Clone)]
pub struct Options {
    pub sora_api_url: String,
    pub polling_interval: std::num::NonZeroUsize,
    pub chart_time_period: std::num::NonZeroUsize,
    pub connection_filter: regex::Regex,
    pub stats_key_filter: regex::Regex,
    pub record: Option<PathBuf>,
}

impl Options {
    fn create_recorder(&self) -> orfail::Result<Option<BufWriter<File>>> {
        if let Some(path) = &self.record {
            let file = File::create(path)
                .or_fail_with(|e| format!("failed to create record file {path:?}: {e}"))?;
            Ok(Some(BufWriter::new(file)))
        } else {
            Ok(None)
        }
    }

    fn is_realtime_mode(&self) -> bool {
        self.sora_api_url.starts_with("http://") || self.sora_api_url.starts_with("https://")
    }
}
