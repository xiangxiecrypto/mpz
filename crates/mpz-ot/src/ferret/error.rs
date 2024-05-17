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
