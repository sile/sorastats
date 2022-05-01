use anyhow::Context;
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ConnectionStatsValue {
    pub value: StatsValue,
    pub delta_per_sec: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct AggregatedStatsValue {
    pub value_sum: Option<f64>,
    pub delta_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct AggregatedStats {
    pub stats: BTreeMap<StatsKey, AggregatedStatsValue>,
}

impl AggregatedStats {
    fn new(connections: &[ConnectionStats2]) -> Self {
        let mut keys = BTreeSet::new();
        let mut sums = BTreeMap::<_, f64>::new();
        let mut deltas = BTreeMap::<_, f64>::new();

        for conn in connections {
            for (k, item) in &conn.stats {
                keys.insert(k);
                if let Some(v) = item.value.as_f64() {
                    *sums.entry(k).or_default() += v;
                }
                if let Some(delta) = item.delta_per_sec {
                    *deltas.entry(k).or_default() += delta;
                }
            }
        }

        let stats = keys
            .into_iter()
            .map(|k| {
                let v = AggregatedStatsValue {
                    value_sum: sums.get(k).copied(),
                    delta_per_sec: deltas.get(k).copied(),
                };
                (k.to_owned(), v)
            })
            .collect();
        Self { stats }
    }
}

pub type StatsKey = String; // TODO: StatsItemKey
pub type ConnectionId = String;

// TODO: rename
#[derive(Debug, Clone)]
pub struct Stats2 {
    pub time: SystemTime,
    pub aggregated: AggregatedStats,
    pub connections: BTreeMap<ConnectionId, ConnectionStats2>,
}

impl Stats2 {
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn item_count(&self) -> usize {
        self.aggregated.stats.len()
    }

    // TODO: BTreeMap
    pub fn new(connections: Vec<ConnectionStats2>) -> Self {
        let aggregated = AggregatedStats::new(&connections);
        let connections = connections
            .into_iter()
            .map(|c| (c.connection_id.clone(), c))
            .collect();
        Self {
            time: SystemTime::now(),
            aggregated,
            connections,
        }
    }

    pub fn empty() -> Self {
        Self {
            time: SystemTime::now(),
            aggregated: Default::default(),
            connections: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionStats2 {
    pub connection_id: ConnectionId,
    pub timestamp: chrono::DateTime<chrono::FixedOffset>,
    pub stats: BTreeMap<String, ConnectionStatsValue>,
}

impl ConnectionStats2 {
    pub fn new(json: serde_json::Value, prev: &Stats2) -> anyhow::Result<Self> {
        let obj = json
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("not a JSON object"))?;
        let connection_id = obj
            .get("connection_id")
            .ok_or_else(|| anyhow::anyhow!("missing 'connection_id'"))?
            .as_str()
            .expect("TODO")
            .to_owned();
        let timestamp = obj
            .get("timestamp")
            .ok_or_else(|| anyhow::anyhow!("missing 'timestamp'"))?
            .as_str()
            .expect("TODO")
            .to_owned();
        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp)
            .with_context(|| format!("parse timestamp failed: {:?}", timestamp))?;

        let mut key = String::new();
        let mut stats = Stats::new();
        collect_stats(obj, &mut stats, &mut key);

        let duration = prev
            .connections
            .get(&connection_id)
            .map(|c| (timestamp - c.timestamp).to_std().expect("TODO"));
        let stats = stats
            .into_iter()
            .map(|(k, v)| {
                let delta_per_sec = if let Some(d) = duration {
                    prev.connections[&connection_id]
                        .stats
                        .get(&k)
                        .and_then(|x| match (v.as_f64(), x.value.as_f64()) {
                            (Some(v1), Some(v0)) => Some(v1 - v0 / d.as_secs_f64()),
                            _ => None,
                        })
                } else {
                    None
                };
                let v = ConnectionStatsValue {
                    value: v,
                    delta_per_sec,
                };
                (k, v)
            })
            .collect();
        Ok(Self {
            connection_id,
            timestamp,
            stats,
        })
    }
}

pub type Stats = BTreeMap<String, StatsValue>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StatsValue {
    Number(OrderedFloat<f64>),
    Bool(bool),
    String(String),
}

impl StatsValue {
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Number(v) = self {
            Some(v.0)
        } else {
            None
        }
    }
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
