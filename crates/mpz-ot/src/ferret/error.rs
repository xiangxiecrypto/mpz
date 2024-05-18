use crate::OTError;

/// A Ferret sender error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::error::SenderError),
    #[error(transparent)]
    MPCOTSenderError(#[from] crate::ferret::mpcot::SenderError),
    #[error(transparent)]
    MPCOTSenderRegularError(#[from] crate::ferret::mpcot::SenderRegularError),
    #[error(transparent)]
    RandomCOTError(#[from] OTError),
    #[error("{0}")]
    StateError(String),
    #[error("{0}")]
    MPCOTSenderTypeError(String),
}

impl From<SenderError> for OTError {
    fn from(err: SenderError) -> Self {
        match err {
            SenderError::IOError(e) => e.into(),
            e => OTError::SenderError(Box::new(e)),
        }
    }
}

impl From<crate::ferret::sender::StateError> for SenderError {
    fn from(err: crate::ferret::sender::StateError) -> Self {
        SenderError::StateError(err.to_string())
    }
}

impl<RandomCOT> From<crate::ferret::sender::MpcotSenderError<RandomCOT>> for SenderError {
    fn from(err: crate::ferret::sender::MpcotSenderError<RandomCOT>) -> Self {
        SenderError::MPCOTSenderTypeError(err.to_string())
    }
}

/// A Ferret receiver error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::error::ReceiverError),
    #[error(transparent)]
    MPCOTReceiverError(#[from] crate::ferret::mpcot::ReceiverError),
    #[error(transparent)]
    MPCOTReceiverRegularError(#[from] crate::ferret::mpcot::ReceiverRegularError),
    #[error(transparent)]
    RandomCOTError(#[from] OTError),
    #[error("{0}")]
    StateError(String),
    #[error("{0}")]
    MPCOTReceiverTypeError(String),
}

impl From<ReceiverError> for OTError {
    fn from(err: ReceiverError) -> Self {
        match err {
            ReceiverError::IOError(e) => e.into(),
            e => OTError::ReceiverError(Box::new(e)),
        }
    }
}

impl From<crate::ferret::receiver::StateError> for ReceiverError {
    fn from(err: crate::ferret::receiver::StateError) -> Self {
        ReceiverError::StateError(err.to_string())
    }
}

impl<RandomCOT> From<crate::ferret::receiver::MpcotReceiverError<RandomCOT>> for ReceiverError {
    fn from(err: crate::ferret::receiver::MpcotReceiverError<RandomCOT>) -> Self {
        ReceiverError::MPCOTReceiverTypeError(err.to_string())
    }
}
