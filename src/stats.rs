use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatsKey {
    Total,
    Channel(String),
    Bundle(String),
    Client(String),
    Connection(String),
}

pub type StatsMap = BTreeMap<StatsKey, Stats>;

#[derive(Debug, Clone)]
pub struct Stats {}

#[derive(Debug, Clone)]
pub struct ConnectionStats {}

impl ConnectionStats {
    pub fn from_json(value: serde_json::Value) -> anyhow::Result<Self> {
        todo!()
    }
}
