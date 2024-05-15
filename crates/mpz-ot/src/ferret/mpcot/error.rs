/// A MPCOT sender error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]

pub enum SenderError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::mpcot::error::SenderError),
    #[error(transparent)]
    SPCOTSenderError(#[from] crate::ferret::spcot::SenderError),
    #[error("{0}")]
    StateError(String),
}

impl From<crate::ferret::mpcot::sender::StateError> for SenderError {
    fn from(err: crate::ferret::mpcot::sender::StateError) -> Self {
        SenderError::StateError(err.to_string())
    }
}
