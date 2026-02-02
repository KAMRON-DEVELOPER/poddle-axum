use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct PrometheusConfig {
    pub url: String,
    pub scrape_interval_seconds: u64,
    pub metric_snapshots_to_keep: i64,
    #[serde(default = "rate_default")]
    pub rate: String,
}

fn rate_default() -> String {
    String::from("1m")
}
