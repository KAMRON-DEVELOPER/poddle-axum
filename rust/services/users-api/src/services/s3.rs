use object_store::aws::{AmazonS3, AmazonS3Builder};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct S3ServiceConfig {
    pub access_key_id: String,
    pub secret_key: String,
    pub url: String,
    pub region: String,
    pub bucket_name: String,
    pub allow_http: bool,
}

pub struct S3Service {}

pub fn build_s3(cfg: &S3ServiceConfig) -> AmazonS3 {
    AmazonS3Builder::new()
        .with_access_key_id(cfg.access_key_id.clone())
        .with_secret_access_key(cfg.secret_key.clone())
        .with_url(cfg.url.clone())
        .with_region(cfg.region.clone())
        .with_bucket_name(cfg.bucket_name.clone())
        .with_allow_http(cfg.allow_http)
        .build()
        .expect("Failed to build s3")
}
