use crate::OTError;

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

impl From<SenderError> for OTError {
    fn from(err: SenderError) -> Self {
        match err {
            SenderError::IOError(e) => e.into(),
            e => OTError::SenderError(Box::new(e)),
        }
    }
}

// impl From<crate::ferret::mpcot::sender::StateError> for SenderError {
//     fn from(err: crate::ferret::mpcot::sender::StateError) -> Self {
//         SenderError::StateError(err.to_string())
//     }
// }

impl From<crate::ferret::mpcot::sender::StateError> for SenderError {
    fn from(err: crate::ferret::mpcot::sender::StateError) -> Self {
        SenderError::StateError(err.to_string())
    }
}

/// A MPCOT receiver error
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::mpcot::error::ReceiverError),
    #[error(transparent)]
    SpcotReceiverError(#[from] crate::ferret::spcot::ReceiverError),
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

// impl From<crate::ferret::mpcot::receiver::StateError> for ReceiverError {
//     fn from(err: crate::ferret::mpcot::receiver::StateError) -> Self {
//         ReceiverError::StateError(err.to_string())
//     }
// }

impl From<crate::ferret::mpcot::receiver::StateError> for ReceiverError {
    fn from(err: crate::ferret::mpcot::receiver::StateError) -> Self {
        ReceiverError::StateError(err.to_string())
    }
}

/// A MPCOT regular sender error.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderRegularError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::mpcot::error::SenderError),
    #[error(transparent)]
    SPCOTSenderError(#[from] crate::ferret::spcot::SenderError),
    #[error("{0}")]
    StateError(String),
}

impl From<SenderRegularError> for OTError {
    fn from(err: SenderRegularError) -> Self {
        match err {
            SenderRegularError::IOError(e) => e.into(),
            e => OTError::SenderError(Box::new(e)),
        }
    }
}

// impl From<crate::ferret::mpcot::sender_regular::StateError> for SenderRegularError {
//     fn from(err: crate::ferret::mpcot::sender_regular::StateError) -> Self {
//         SenderRegularError::StateError(err.to_string())
//     }
// }

/// A MPCOT regular receiver error
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverRegularError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    CoreError(#[from] mpz_ot_core::ferret::mpcot::error::ReceiverError),
    #[error(transparent)]
    SpcotReceiverError(#[from] crate::ferret::spcot::ReceiverError),
    #[error("{0}")]
    StateError(String),
}

impl From<ReceiverRegularError> for OTError {
    fn from(err: ReceiverRegularError) -> Self {
        match err {
            ReceiverRegularError::IOError(e) => e.into(),
            e => OTError::ReceiverError(Box::new(e)),
        }
    }
}

// impl From<crate::ferret::mpcot::receiver_regular::StateError> for ReceiverRegularError {
//     fn from(err: crate::ferret::mpcot::receiver_regular::StateError) -> Self {
//         ReceiverRegularError::StateError(err.to_string())
//     }
// }
