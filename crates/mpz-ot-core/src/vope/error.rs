//! Errors that can occur when using VOPE.

/// Errors that can occur when using VOPE sender (verifier).
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderError {
    #[error("invalid input: expected {0}")]
    InvalidInput(String),
    #[error("invalid length: expected {0}")]
    InvalidLength(String),
}

/// Errors that can occur when using VOPE receiver (prover).
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverError {
    #[error("invalid input: expected {0}")]
    InvalidInput(String),
    #[error("invalid length: expected {0}")]
    InvalidLength(String),
}
