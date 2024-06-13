//! Errors in VOPE

use crate::VOPEError;

/// A VOPE Sender error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_zk_core::vope::error::SenderError),
    #[error(transparent)]
    RandomCOTError(#[from] mpz_ot::OTError),
    #[error("{0}")]
    StateError(String),
}

/// A VOPE Receiver error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_zk_core::vope::error::ReceiverError),
    #[error(transparent)]
    RandomCOTError(#[from] mpz_ot::OTError),
    #[error("{0}")]
    StateError(String),
}

impl From<SenderError> for VOPEError {
    fn from(err: SenderError) -> Self {
        match err {
            SenderError::IOError(e) => e.into(),
            e => VOPEError::SenderError(Box::new(e)),
        }
    }
}

impl From<crate::vope::sender::StateError> for SenderError {
    fn from(err: crate::vope::sender::StateError) -> Self {
        SenderError::StateError(err.to_string())
    }
}

impl From<ReceiverError> for VOPEError {
    fn from(err: ReceiverError) -> Self {
        match err {
            ReceiverError::IOError(e) => e.into(),
            e => VOPEError::ReceiverError(Box::new(e)),
        }
    }
}

impl From<crate::vope::receiver::StateError> for ReceiverError {
    fn from(err: crate::vope::receiver::StateError) -> Self {
        ReceiverError::StateError(err.to_string())
    }
}
