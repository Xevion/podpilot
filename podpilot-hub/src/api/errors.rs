//! Error types for the API client module.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiClientError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] anyhow::Error),
}