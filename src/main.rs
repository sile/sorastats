use anyhow::Context;
use clap::Parser;
use sorastats::{poll, ui};
use std::path::PathBuf;

/// WebRTC SFU Sora の統計情報のコマンドラインビューア
#[derive(Debug, Parser)]
#[clap(version)]
struct Args {
    #[clap(flatten)]
    options: sorastats::Options,

    #[clap(hide = true, long)]
    logfile: Option<PathBuf>,

    #[clap(hide = true,long, default_value_t = simplelog::LevelFilter::Info)]
    loglevel: simplelog::LevelFilter,

    #[clap(hide = true, long)]
    truncate_log: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    setup_logger(&args)?;

    let rx = poll::StatsPollingOptions::new(args.options.clone()).start_polling_thread()?;
    let app = ui::App::new(rx, args.options)?;
    app.run()
}

fn setup_logger(args: &Args) -> anyhow::Result<()> {
    if let Some(logfile) = &args.logfile {
        let file = std::fs::OpenOptions::new()
            .append(!args.truncate_log)
            .truncate(args.truncate_log)
            .create(true)
            .write(true)
            .open(logfile)
            .with_context(|| format!("failed to open log file {:?}", logfile))?;
        simplelog::WriteLogger::init(args.loglevel, Default::default(), file)?;
    }
    Ok(())
}
