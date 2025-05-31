use orfail::OrFail;
use sorastats::{poll, ui};
use std::path::PathBuf;

#[derive(Debug)]
struct Args {
    options: sorastats::Options,
    logfile: Option<PathBuf>,
    loglevel: simplelog::LevelFilter,
    truncate_log: bool,
}

impl Args {
    fn parse() -> noargs::Result<Self> {
        let mut args = noargs::raw_args();

        args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
        args.metadata_mut().app_description = "WebRTC SFU Sora の統計情報ビューア";

        if noargs::VERSION_FLAG.take(&mut args).is_present() {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        noargs::HELP_FLAG.take_help(&mut args);

        let sora_api_url: String = noargs::arg("<SORA_API_URL>")
            .doc("「Sora の API の URL（リアルタイムモード）」あるいは「過去に `--record` で記録したファイルのパス（リプレイモード）」")
            .example("http://localhost:5000/api")
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
            .doc(concat!(
                "集計対象に含めるコネクションをフィルタするための正規表現\n",
                "\n",
                "コネクションの各統計値は '${KEY}:${VALUE}' という形式の文字列に変換された上で、\n",
                "指定の正規表現にマッチ（部分一致）するかどうかがチェックされる。\n",
                "一つでもマッチする統計値が存在する場合には、そのコネクションは集計対象に含まれる。\n",
                "\n",
                "例えば、チャンネル名が 'sora' のコネクションのみを対象にしたい場合には\n",
                "'^channel_id:sora$' という正規表現を指定すると良い。\n",

            ))
            .default(".*:.*")
            .take(&mut args)
            .then(|o| regex::Regex::new(o.value()))?;

        let stats_key_filter: regex::Regex = noargs::opt("stats-key-filter")
            .short('k')
            .doc(concat!(
                "集計対象に含める統計項目をフィルタするための正規表現\n",
                "\n",
                "指定された正規表現にマッチ（部分一致）する統計項目のみが表示される。\n",
                "\n",
                "例えば、 RTP 関連の統計情報のみを対象としたい場合には\n",
                "'^rtp[.]' という正規表現を指定すると良い。\n",
            ))
            .default(".*")
            .take(&mut args)
            .then(|o| regex::Regex::new(o.value()))?;

        let record: Option<PathBuf> = noargs::opt("record")
            .doc(concat!(
                "指定されたファイルに、取得した統計情報を記録する\n",
                "\n",
                "`<SORA_API_URL>`引数に URL の代わりにこのファイルへのパスを指定することで、\n",
                "記録した統計情報を後から閲覧することができる\n'",
                "\n",
                "リプレイモードの場合には、このオプションを指定しても無視される\n"
            ))
            .take(&mut args)
            .present_and_then(|o| o.value().parse())?;

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
    let args = Args::parse()?;

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
