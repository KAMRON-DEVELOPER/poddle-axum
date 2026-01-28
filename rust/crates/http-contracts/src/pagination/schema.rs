use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub struct Pagination {
    #[serde(default = "default_offset")]
    #[serde_as(as = "DisplayFromStr")]
    pub offset: i64,
    #[serde(default = "default_limit")]
    #[serde_as(as = "DisplayFromStr")]
    pub limit: i64,
}

fn default_offset() -> i64 {
    0
}

fn default_limit() -> i64 {
    20
}
