use crate::OTError;

/// A SPCOT sender error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::spcot::error::SenderError),
    #[error(transparent)]
    RandomCOTError(#[from] OTError),
    #[error("{0}")]
    StateError(String),
}

impl From<SenderError> for OTError {
    fn from(err: SenderError) -> Self {
        match err {
            SenderError::IOError(e) => e.into(),
            e => OTError::SenderError(Box::new(e)),
        }
    }
}

impl From<crate::ferret::spcot::sender::StateError> for SenderError {
    fn from(err: crate::ferret::spcot::sender::StateError) -> Self {
        SenderError::StateError(err.to_string())
    }
}

/// A SPCOT receiver error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::spcot::error::ReceiverError),
    #[error(transparent)]
    RandomCOTError(#[from] OTError),
    #[error("{0}")]
    StateError(String),
}

impl From<ReceiverError> for OTError {
    fn from(err: ReceiverError) -> Self {
        match err {
            ReceiverError::IOError(e) => e.into(),
            e => OTError::ReceiverError(Box::new(e)),
        }
    }
}

impl From<crate::ferret::spcot::receiver::StateError> for ReceiverError {
    fn from(err: crate::ferret::spcot::receiver::StateError) -> Self {
        ReceiverError::StateError(err.to_string())
    }
}
