use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// #[serde_as]
// #[derive(Deserialize, Serialize, JsonSchema, Debug)]
// pub struct Pagination {
//     #[serde(default = "default_offset")]
//     #[serde_as(as = "DisplayFromStr")]
//     #[schemars(with = "i64")]
//     pub offset: i64,
//     #[serde(default = "default_limit")]
//     #[serde_as(as = "DisplayFromStr")]
//     #[schemars(with = "i64")]
//     pub limit: i64,
// }

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct Pagination {
    #[serde(default = "default_offset")]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_offset() -> i64 {
    0
}

fn default_limit() -> i64 {
    20
}
