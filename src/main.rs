use clap::Parser;
use orfail::OrFail;
use sorastats::{poll, ui};
use std::path::PathBuf;

/// WebRTC SFU Sora の統計情報ビューア
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

impl Args {
    fn parse_with_noargs() -> noargs::Result<Self> {
        let mut args = noargs::raw_args();

        // Set metadata for help
        args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
        args.metadata_mut().app_description = "WebRTC SFU Sora の統計情報ビューア";

        // Handle well-known flags
        if noargs::VERSION_FLAG.take(&mut args).is_present() {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        noargs::HELP_FLAG.take_help(&mut args);

        // Parse options fields (from sorastats::Options)
        let sora_api_url: String = noargs::arg("<SORA_API_URL>")
        .doc("「Sora の API の URL（リアルタイムモード）」あるいは「過去に `--record` で記録したファイルのパス（リプレイモード）」")
        .take(&mut args)
        .then(|a| a.value().parse())?;

        let polling_interval: std::num::NonZeroUsize = noargs::opt("polling-interval")
            .short('i')
            .doc("統計 API から情報を取得する間隔（秒単位）")
            .default("1")
            .take(&mut args)
            .then(|o| o.value().parse())?;

        let chart_time_period: std::num::NonZeroUsize = noargs::opt("chart-time-period")
            .short('p')
            .doc("チャートの X 軸の表示期間（秒単位）")
            .default("60")
            .take(&mut args)
            .then(|o| o.value().parse())?;

        let connection_filter: regex::Regex = noargs::opt("connection-filter")
            .short('c')
            .doc("集計対象に含めるコネクションをフィルタするための正規表現")
            .default(".*:.*")
            .take(&mut args)
            .then(|o| regex::Regex::new(o.value()))?;

        let stats_key_filter: regex::Regex = noargs::opt("stats-key-filter")
            .short('k')
            .doc("集計対象に含める統計項目をフィルタするための正規表現")
            .default(".*")
            .take(&mut args)
            .then(|o| regex::Regex::new(o.value()))?;

        let record: Option<PathBuf> = noargs::opt("record")
            .doc("指定されたファイルに、取得した統計情報を記録する")
            .take(&mut args)
            .present_and_then(|o| o.value().parse())?;

        // Parse hidden Args fields
        let logfile: Option<PathBuf> = noargs::opt("logfile")
            .take(&mut args)
            .present_and_then(|o| o.value().parse())?;

        let loglevel: simplelog::LevelFilter = noargs::opt("loglevel")
            .default("Info")
            .take(&mut args)
            .then(|o| o.value().parse())?;

        let truncate_log: bool = noargs::flag("truncate-log").take(&mut args).is_present();

        // Check for unexpected args and build help if needed
        if let Some(help) = args.finish()? {
            print!("{}", help);
            std::process::exit(0);
        }

        Ok(Args {
            options: sorastats::Options {
                sora_api_url,
                polling_interval,
                chart_time_period,
                connection_filter,
                stats_key_filter,
                record,
            },
            logfile,
            loglevel,
            truncate_log,
        })
    }
}

fn main() -> noargs::Result<()> {
    let args = Args::parse();

    setup_logger(&args).or_fail()?;

    let rx = poll::StatsPoller::start_thread(args.options.clone()).or_fail()?;
    let app = ui::App::new(rx, args.options).or_fail()?;
    let result = app.run().or_fail();
    if let Err(e) = &result {
        log::error!("{}", e);
        println!();
    }
    result?;
    Ok(())
}

fn setup_logger(args: &Args) -> orfail::Result<()> {
    if let Some(logfile) = &args.logfile {
        let file = std::fs::OpenOptions::new()
            .append(!args.truncate_log)
            .truncate(args.truncate_log)
            .create(true)
            .write(true)
            .open(logfile)
            .or_fail_with(|e| format!("failed to open log file {logfile:?}: {e}"))?;
        simplelog::WriteLogger::init(args.loglevel, Default::default(), file).or_fail()?;
    }
    Ok(())
}
