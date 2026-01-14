use crate::config::Config;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};

pub fn build_s3(config: &Config) -> AmazonS3 {
    AmazonS3Builder::new()
        .with_region(config.s3_region.clone().unwrap())
        .with_bucket_name(config.s3_bucket_name.clone().unwrap())
        .with_access_key_id(config.s3_access_key_id.clone().unwrap())
        .with_secret_access_key(config.s3_secret_key.clone().unwrap())
        .with_url(config.s3_endpoint.clone().unwrap())
        .build()
        .expect("Failed to build s3")
}

pub fn build_gcs(config: &Config) ->  GoogleCloudStorage  {
    if let Some(binding) = config.gcp_service_account_path.clone()
        && binding.exists()
    {
        let service_account_path = binding.to_str().unwrap();

        return  GoogleCloudStorageBuilder::new()
            .with_service_account_path(service_account_path)
            .with_bucket_name(config.s3_bucket_name.clone().unwrap())
            // .with_url(format!("gs://{}", config.s3_bucket_name.clone().unwrap()))
            .build().unwrap_or_else(|e| {panic!("Couldn't establish Google Cloud Storage connection: {}", e)});
    }

    panic!("Missing GCP Credentials")
}
