use anyhow::Context;
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Instant, SystemTime};

pub type StatsItemKey = String;
pub type ConnectionId = String;

#[derive(Debug, Clone)]
pub struct ConnectionStatsItemValue {
    pub value: StatsItemValue,
    pub delta_per_sec: Option<f64>,
}

impl ConnectionStatsItemValue {
    pub fn format_value(&self) -> String {
        if let StatsItemValue::Number(v) = self.value {
            format_u64(v.0 as u64)
        } else {
            self.value.to_string()
        }
    }

    pub fn format_delta_per_sec(&self) -> String {
        if let Some(v) = self.delta_per_sec {
            format_u64(v.round() as u64)
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone)]
pub struct AggregatedStatsItemValue {
    pub value_sum: Option<f64>,
    pub delta_per_sec: Option<f64>,
}

impl AggregatedStatsItemValue {
    pub fn format_value_sum(&self) -> String {
        if let Some(v) = self.value_sum {
            format_u64(v.round() as u64)
        } else {
            String::new()
        }
    }

    pub fn format_delta_per_sec(&self) -> String {
        if let Some(v) = self.delta_per_sec {
            format_u64(v.round() as u64)
        } else {
            String::new()
        }
    }
}

pub fn format_u64(mut n: u64) -> String {
    let mut s = Vec::new();
    for i in 0.. {
        if i % 3 == 0 && i != 0 {
            s.push(b',');
        }
        let m = n % 10;
        s.push(b'0' + m as u8);
        n /= 10;
        if n == 0 {
            break;
        }
    }
    s.reverse();
    String::from_utf8(s).expect("unreachable")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StatsItemValue {
    Number(OrderedFloat<f64>),
    Bool(bool),
    String(String),
}

impl StatsItemValue {
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Number(v) = self {
            Some(v.0)
        } else {
            None
        }
    }
}

impl std::fmt::Display for StatsItemValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Number(x) => write!(f, "{x}"),
            Self::Bool(x) => write!(f, "{x}"),
            Self::String(x) => write!(f, "{x}"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AggregatedStats {
    pub items: BTreeMap<StatsItemKey, AggregatedStatsItemValue>,
}

impl AggregatedStats {
    fn new(connections: &[ConnectionStats]) -> Self {
        let mut keys = BTreeSet::new();
        let mut sums = BTreeMap::<_, f64>::new();
        let mut deltas = BTreeMap::<_, f64>::new();

        for conn in connections {
            for (k, item) in &conn.items {
                keys.insert(k);
                if let Some(v) = item.value.as_f64() {
                    *sums.entry(k).or_default() += v;
                }
                if let Some(delta) = item.delta_per_sec {
                    *deltas.entry(k).or_default() += delta;
                }
            }
        }

        let items = keys
            .into_iter()
            .map(|k| {
                let v = AggregatedStatsItemValue {
                    value_sum: sums.get(k).copied(),
                    delta_per_sec: deltas.get(k).copied(),
                };
                (k.to_owned(), v)
            })
            .collect();
        Self { items }
    }
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub time: SystemTime,
    pub timestamp: Instant,
    pub aggregated: AggregatedStats,
    pub connections: BTreeMap<ConnectionId, ConnectionStats>,
}

impl Stats {
    pub fn new(connections: Vec<ConnectionStats>) -> Self {
        let aggregated = AggregatedStats::new(&connections);
        let connections = connections
            .into_iter()
            .map(|c| (c.connection_id.clone(), c))
            .collect();
        Self {
            time: SystemTime::now(),
            timestamp: Instant::now(),
            aggregated,
            connections,
        }
    }

    pub fn empty() -> Self {
        Self {
            time: SystemTime::now(),
            timestamp: Instant::now(),
            aggregated: Default::default(),
            connections: Default::default(),
        }
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn item_count(&self) -> usize {
        self.aggregated.items.len()
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub connection_id: ConnectionId,
    pub timestamp: chrono::DateTime<chrono::FixedOffset>,
    pub items: BTreeMap<StatsItemKey, ConnectionStatsItemValue>,
}

impl ConnectionStats {
    pub fn new(json: serde_json::Value, prev: &Stats) -> anyhow::Result<Self> {
        let obj = json
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("not a JSON object"))?;
        let connection_id = obj
            .get("connection_id")
            .ok_or_else(|| anyhow::anyhow!("missing 'connection_id'"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("not a JSON string"))?
            .to_owned();
        let timestamp = obj
            .get("timestamp")
            .ok_or_else(|| anyhow::anyhow!("missing 'timestamp'"))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("not a JSON string"))?
            .to_owned();
        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp)
            .with_context(|| format!("parse timestamp failed: {:?}", timestamp))?;

        let mut key = String::new();
        let mut stats_items = BTreeMap::new();
        collect_stats_items(obj, &mut stats_items, &mut key);

        let duration = prev
            .connections
            .get(&connection_id)
            .map(|c| (timestamp - c.timestamp).to_std())
            .transpose()?;
        let items = stats_items
            .into_iter()
            .map(|(k, v)| {
                let delta_per_sec = if let Some(d) = duration {
                    prev.connections[&connection_id]
                        .items
                        .get(&k)
                        .and_then(|x| match (v.as_f64(), x.value.as_f64()) {
                            (Some(v1), Some(v0)) => Some((v1 - v0) / d.as_secs_f64()),
                            _ => None,
                        })
                } else {
                    None
                };
                let v = ConnectionStatsItemValue {
                    value: v,
                    delta_per_sec,
                };
                (k, v)
            })
            .collect();
        Ok(Self {
            connection_id,
            timestamp,
            items,
        })
    }
}

fn collect_stats_items(
    obj: &serde_json::Map<String, serde_json::Value>,
    items: &mut BTreeMap<StatsItemKey, StatsItemValue>,
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
                    items.insert(key.clone(), StatsItemValue::Number(OrderedFloat(v)));
                } else {
                    log::warn!("too large number (ignored): {v}");
                }
            }
            serde_json::Value::Bool(v) => {
                items.insert(key.clone(), StatsItemValue::Bool(*v));
            }
            serde_json::Value::String(v) => {
                items.insert(key.clone(), StatsItemValue::String(v.clone()));
            }
            serde_json::Value::Object(children) => {
                collect_stats_items(children, items, key);
            }
            _ => {
                log::warn!("unexpected stats value (ignored): {v}");
            }
        };
        key.truncate(old_len);
    }
}
