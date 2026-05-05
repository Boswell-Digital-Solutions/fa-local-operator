use thiserror::Error;

use crate::domain::guards::DenialGuard;

pub type FaLocalResult<T> = Result<T, FaLocalError>;

#[derive(Debug, Error)]
pub enum FaLocalError {
    #[error(transparent)]
    Denied(#[from] DenialGuard),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("contract invalid: {0}")]
    ContractInvalid(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("schema compilation failed for {schema}: {message}")]
    SchemaCompile { schema: String, message: String },

    #[error("schema validation failed for {schema}: {errors:?}")]
    SchemaValidation { schema: String, errors: Vec<String> },

    #[error("internal invariant violated: {0}")]
    InternalInvariant(String),

    #[error("writeback not yet wired: {0}")]
    WritebackNotWired(String),
}
