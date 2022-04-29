use chrono::DateTime;
use chrono::FixedOffset;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatsKey {
    Total,
    Channel(String),
    Client(String),
    Bundle(String),
    Connection(String),
}

pub type StatsMap = BTreeMap<StatsKey, Stats>;

pub type Counters = BTreeMap<String, u64>;

#[derive(Debug, Clone)]
pub struct Stats {}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub timestamp: DateTime<FixedOffset>,
    pub channel_id: String,
    pub client_id: String,
    pub bundle_id: String,
    pub connection_id: String,
    pub counters: Counters,
}

impl ConnectionStats {
    pub fn from_json(value: serde_json::Value) -> anyhow::Result<Self> {
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("not a JSON object"))?;

        let timestamp = DateTime::parse_from_rfc3339(&get_string(obj, "timestamp")?)?;

        let channel_id = get_string(obj, "channel_id")?;
        let client_id = get_string(obj, "client_id")?;
        let connection_id = get_string(obj, "connection_id")?;
        let bundle_id = get_string(obj, "bundle_id")
            .ok()
            .unwrap_or_else(|| connection_id.clone());

        let mut key = String::new();
        let mut counters = Counters::new();
        collect_positive_counters(obj, &mut counters, &mut key);
        Ok(Self {
            timestamp,
            channel_id,
            client_id,
            connection_id,
            bundle_id,
            counters,
        })
    }
}

fn collect_positive_counters(
    obj: &serde_json::Map<String, serde_json::Value>,
    counters: &mut Counters,
    key: &mut String,
) {
    for (k, v) in obj {
        if k == "unstable_level" {
            // Not a counter.
            continue;
        }

        let old_len = key.len();
        if !key.is_empty() {
            key.push('.');
        }
        key.push_str(k);
        match v {
            serde_json::Value::Number(v) => {
                if let Some(v) = v.as_u64() {
                    if v > 0 {
                        counters.insert(key.clone(), v);
                    }
                }
            }
            serde_json::Value::Object(children) => {
                collect_positive_counters(children, counters, key);
            }
            _ => {}
        }
        key.truncate(old_len);
    }
}

fn get_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> anyhow::Result<String> {
    let value = obj
        .get(key)
        .ok_or_else(|| anyhow::anyhow!("missing {key:?}"))?;
    Ok(value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("{key:?} is not a JSON string"))?
        .to_owned())
}
