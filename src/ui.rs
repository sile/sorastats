use crate::poller::StatsReceiver;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct UiOpts {
    #[clap(long, default_value_t = 600.0)]
    pub retention_period: f64,

    #[clap(long, default_value = "total")]
    pub tab: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tab {
    Total,
    Channel(String),
    Client(String),
    Bundle(String),
    Connection(String),
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Total => write!(f, "total"),
            Self::Channel(v) => write!(f, "channel:{v}"),
            Self::Client(v) => write!(f, "client:{v}"),
            Self::Bundle(v) => write!(f, "bundle:{v}"),
            Self::Connection(v) => write!(f, "connection:{v}"),
        }
    }
}

impl std::str::FromStr for Tab {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "total" {
            Ok(Self::Total)
        } else if s.starts_with("channel:") {
            Ok(Self::Channel(s["channel:".len()..].to_owned()))
        } else if s.starts_with("client:") {
            Ok(Self::Client(s["client:".len()..].to_owned()))
        } else if s.starts_with("bundle:") {
            Ok(Self::Bundle(s["bundle:".len()..].to_owned()))
        } else if s.starts_with("connection:") {
            Ok(Self::Connection(s["connection:".len()..].to_owned()))
        } else {
            anyhow::bail!("invalid tab name {s:?}");
        }
    }
}

#[derive(Debug)]
pub struct App {
    rx: StatsReceiver,
    opt: UiOpts,
}

impl App {
    pub fn new(rx: StatsReceiver, opt: UiOpts) -> Self {
        Self { rx, opt }
    }

    pub fn run(self) -> anyhow::Result<()> {
        todo!()
    }
}
