use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, Debug)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
}
