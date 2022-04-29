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
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    setup_logger(&args)?;

    let x: serde_json::Value = ureq::post(&args.polling_opt.sora_url)
        .set("x-sora-target", "Sora_20171101.GetStatsAllConnections")
        .call()?
        .into_json()?;
    println!("{}", x);
    Ok(())
}

fn setup_logger(args: &Args) -> anyhow::Result<()> {
    if let Some(logfile) = &args.logfile {
        let file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .write(true)
            .open(logfile)
            .with_context(|| format!("failed to open log file {:?}", logfile))?;
        simplelog::WriteLogger::init(args.loglevel, Default::default(), file)?;
    }
    Ok(())
}
