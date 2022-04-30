use ordered_float::OrderedFloat;
use std::collections::BTreeMap;

// TODO: delete
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatsKey {
    Total,
    Channel(String),
    Client(String),
    Bundle(String),
    Connection(String),
}

// pub type StatsMap = BTreeMap<StatsKey, Stats>;

pub type Counters = BTreeMap<String, u64>;

pub type Stats = BTreeMap<String, StatsValue>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StatsValue {
    Number(OrderedFloat<f64>),
    Bool(bool),
    String(String),
}

impl std::fmt::Display for StatsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Number(x) => write!(f, "{x}"),
            Self::Bool(x) => write!(f, "{x}"),
            Self::String(x) => write!(f, "{x}"),
        }
    }
}

// TODO: remove
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub stats: Stats,
}

impl ConnectionStats {
    pub fn from_json(value: serde_json::Value) -> anyhow::Result<Self> {
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("not a JSON object"))?;
        let mut key = String::new();
        let mut stats = Stats::new();
        collect_stats(obj, &mut stats, &mut key);
        Ok(Self { stats })
    }
}

fn collect_stats(
    obj: &serde_json::Map<String, serde_json::Value>,
    stats: &mut Stats,
    key: &mut String,
) {
    for (k, v) in obj {
        let old_len = key.len();
        if !key.is_empty() {
            key.push('.');
        }
        key.push_str(k);
        match v {
            serde_json::Value::Number(v) => {
                if let Some(v) = v.as_f64() {
                    stats.insert(key.clone(), StatsValue::Number(OrderedFloat(v)));
                } else {
                    log::warn!("too large number (ignored): {v}");
                }
            }
            serde_json::Value::Bool(v) => {
                stats.insert(key.clone(), StatsValue::Bool(*v));
            }
            serde_json::Value::String(v) => {
                stats.insert(key.clone(), StatsValue::String(v.clone()));
            }
            serde_json::Value::Object(children) => {
                collect_stats(children, stats, key);
            }
            _ => {
                log::warn!("unexpected stats value (ignored): {v}");
            }
        };
        key.truncate(old_len);
    }
}
