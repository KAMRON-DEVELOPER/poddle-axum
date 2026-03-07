use bigdecimal::BigDecimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    pub amount: BigDecimal,
    pub currency: String,
}
