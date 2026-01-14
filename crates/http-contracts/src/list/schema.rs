use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
}
