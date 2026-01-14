use thiserror::Error;

#[derive(Error, Debug)]
pub enum KafkaError {
    #[error("Kafka error, {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
}
