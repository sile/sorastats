use anyhow::Context;
use clap::Parser;
use sorastats::poller;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(version)]
struct Args {
    #[clap(flatten)]
    polling_opt: poller::StatsPollingOptions,

    #[clap(long)]
    logfile: Option<PathBuf>,

    #[clap(long, default_value_t = simplelog::LevelFilter::Info, possible_values = ["DEBUG", "INFO", "WARN", "ERROR"])]
    loglevel: simplelog::LevelFilter,

    #[clap(long)]
    truncate_log: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    setup_logger(&args)?;

    let rx = args.polling_opt.start_polling_thread()?;
    loop {
        rx.recv()?;
    }

    Ok(())
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
