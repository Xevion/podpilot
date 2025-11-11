//! Main API client implementation.

use std::sync::Arc;

use crate::api::{
    errors::ApiClientError, json::parse_json_with_context, middleware::TransparentMiddleware,
};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde_json;
use tracing::{debug, error, info, trace, warn};

/// Main API client.
pub struct ApiClient {
    http: ClientWithMiddleware,
}

#[allow(dead_code)]
impl ApiClient {
    /// Creates a new API client.
    pub fn new() -> Result<Self> {
        let http = ClientBuilder::new(
            Client::builder()
                .tcp_keepalive(Some(std::time::Duration::from_secs(60 * 5)))
                .read_timeout(std::time::Duration::from_secs(10))
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .context("Failed to create HTTP client")?,
        )
        .with(TransparentMiddleware)
        .build();

        Ok(Self { http })
    }
}
