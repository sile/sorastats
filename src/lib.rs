use orfail::OrFail;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

pub mod poll;
pub mod stats;
pub mod ui;

#[derive(Debug, Clone, clap::Parser)]
pub struct Options {
    /// 「Sora の API の URL（リアルタイムモード）」あるいは「過去に `--record` で記録したファイルのパス（リプレイモード）」
    pub sora_api_url: String,

    /// 統計 API から情報を取得する間隔（秒単位）
    #[clap(long, short = 'i', default_value = "1")]
    pub polling_interval: std::num::NonZeroUsize,

    /// チャートの X 軸の表示期間（秒単位）
    #[clap(long, short = 'p', default_value = "60")]
    pub chart_time_period: std::num::NonZeroUsize,

    /// 集計対象に含めるコネクションをフィルタするための正規表現
    ///
    /// コネクションの各統計値は "${KEY}:${VALUE}" という形式の文字列に変換された上で、
    /// 指定の正規表現にマッチ（部分一致）するかどうかがチェックされる。
    /// 一つでもマッチする統計値が存在する場合には、そのコネクションは集計対象に含まれる。
    ///
    /// 例えば、チャンネル名が "sora" のコネクションのみを対象にしたい場合には
    /// "^channel_id:sora$" という正規表現を指定すると良い。
    #[clap(long, short = 'c', default_value = ".*:.*")]
    pub connection_filter: regex::Regex,

    /// 集計対象に含める統計項目をフィルタするための正規表現
    ///
    /// 指定された正規表現にマッチ（部分一致）する統計項目のみが表示される。
    ///
    /// 例えば、 RTP 関連の統計情報のみを対象としたい場合には
    /// "^rtp[.]" という正規表現を指定すると良い。
    #[clap(long, short = 'k', default_value = ".*")]
    pub stats_key_filter: regex::Regex,

    /// 指定されたファイルに、取得した統計情報を記録する
    ///
    ///
    /// `<SORA_API_URL>`引数に URL の代わりにこのファイルへのパスを指定することで、
    /// 記録した統計情報を後から閲覧することができる
    ///
    /// リプレイモードの場合には、このオプションを指定しても無視される
    #[clap(long)]
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
