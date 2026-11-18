use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{
    config::{retry::RetryConfig, Credentials},
    primitives::ByteStream,
    presigning::PresigningConfig,
    Client,
};
use std::{env, time::Duration};

#[derive(Clone)]
pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    pub async fn from_env() -> Result<Self> {
        let endpoint = env::var("S3_ENDPOINT").context("S3_ENDPOINT must be set")?;
        let region = env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned());
        let bucket = env::var("S3_BUCKET").context("S3_BUCKET must be set")?;
        let access_key = env::var("S3_ACCESS_KEY_ID").context("S3_ACCESS_KEY_ID must be set")?;
        let secret_key = env::var("S3_SECRET_ACCESS_KEY").context("S3_SECRET_ACCESS_KEY must be set")?;

        let shared = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(Credentials::new(access_key, secret_key, None, None, "atlas-supabase-s3"))
            .load()
            .await;
        let config = aws_sdk_s3::config::Builder::from(&shared)
            .endpoint_url(endpoint)
            .force_path_style(true)
            .retry_config(RetryConfig::standard().with_max_attempts(3))
            .build();

        Ok(Self { client: Client::from_conf(config), bucket })
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .context("Supabase S3 object upload failed")?;
        Ok(())
    }

    pub async fn signed_get_url(&self, key: &str) -> Result<String> {
        let config = PresigningConfig::expires_in(Duration::from_secs(900))?;
        let request = self.client.get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(config)
            .await
            .context("could not create signed media URL")?;
        Ok(request.uri().to_string())
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }
}
