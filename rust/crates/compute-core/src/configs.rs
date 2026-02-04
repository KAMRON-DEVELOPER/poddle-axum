use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct PrometheusConfig {
    pub url: String,
    #[serde(default = "scrape_interval_default")]
    pub scrape_interval: i64,
    #[serde(default = "snapshots_to_keep_default")]
    pub snapshots_to_keep: i64,
    #[serde(default = "rate_default")]
    pub rate: String,
}

fn scrape_interval_default() -> i64 {
    15
}

fn snapshots_to_keep_default() -> i64 {
    240
}

fn rate_default() -> String {
    String::from("1m")
}
