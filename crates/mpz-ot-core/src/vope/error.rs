//! Errors that can occur when using VOPE.

/// Errors that can occur when using VOPE sender (verifier).
#[derive(Debug, thiserror::Error)]
#[error("invalid length: expected {0}")]
pub struct SenderError(pub String);

/// Errors that can occur when using VOPE sender (verifier).
#[derive(Debug, thiserror::Error)]
#[error("invalid length: expected {0}")]
pub struct ReceiverError(pub String);