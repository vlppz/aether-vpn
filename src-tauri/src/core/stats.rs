//! Live traffic counters read from Xray's metrics endpoint (`/debug/vars`).

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficTick {
    pub up: u64,
    pub down: u64,
    pub total_up: u64,
    pub total_down: u64,
}

/// Sums uplink/downlink over all outbounds except the internal DNS one.
pub fn totals(vars: &Value) -> Option<(u64, u64)> {
    let outbound = vars.get("stats")?.get("outbound")?.as_object()?;
    Some(outbound.iter().filter(|(tag, _)| *tag != "dns-out").fold((0, 0), |(u, d), (_, v)| {
        (
            u + v["uplink"].as_u64().unwrap_or(0),
            d + v["downlink"].as_u64().unwrap_or(0),
        )
    }))
}

pub fn spawn(app: AppHandle, metrics_port: u16) -> JoinHandle<()> {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("http client");
        let url = format!("http://127.0.0.1:{metrics_port}/debug/vars");
        let mut base: Option<(u64, u64)> = None;
        let mut last: Option<((u64, u64), Instant)> = None;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Ok(resp) = client.get(&url).send().await else { continue };
            let Ok(vars) = resp.json::<Value>().await else { continue };
            let Some(now) = totals(&vars) else { continue };
            let base = *base.get_or_insert(now);
            let tick = match last {
                Some((prev, at)) => {
                    let secs = at.elapsed().as_secs_f64().max(0.001);
                    TrafficTick {
                        up: (now.0.saturating_sub(prev.0) as f64 / secs) as u64,
                        down: (now.1.saturating_sub(prev.1) as f64 / secs) as u64,
                        total_up: now.0.saturating_sub(base.0),
                        total_down: now.1.saturating_sub(base.1),
                    }
                }
                None => TrafficTick::default(),
            };
            last = Some((now, Instant::now()));
            let _ = app.emit("traffic", tick);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_outbounds() {
        let vars = serde_json::json!({ "stats": { "outbound": {
            "proxy": { "uplink": 10, "downlink": 100 },
            "direct": { "uplink": 1, "downlink": 2 },
            "dns-out": { "uplink": 1000, "downlink": 1000 },
        }}});
        assert_eq!(totals(&vars), Some((11, 102)));
        assert_eq!(totals(&serde_json::json!({})), None);
    }
}
