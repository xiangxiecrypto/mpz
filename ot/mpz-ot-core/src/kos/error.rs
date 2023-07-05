/// Errors that can occur when using the KOS15 sender.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderError {
    #[error("invalid state: expected {0}")]
    InvalidState(String),
    #[error("count mismatch: receiver expected {0}, got {1}")]
    CountMismatch(usize, usize),
    #[error("consistency check failed")]
    ConsistencyCheckFailed,
    #[error("not enough OTs are setup: expected {0}, actual {1}")]
    InsufficientSetup(usize, usize),
}

/// Errors that can occur when using the KOS15 receiver.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReceiverError {
    #[error("invalid state: expected {0}")]
    InvalidState(String),
    #[error("count mismatch: receiver expected {0} but sender sent {1}")]
    CountMismatch(usize, usize),
    #[error("invalid payload")]
    InvalidPayload,
    #[error("receiver commitment not configured")]
    NoReceiverCommit,
}

/// Errors that can occur during verification of the receiver's choices.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum SenderVerifyError {
    #[error("number of choices does not match the tape: recorded {0}, got {1}")]
    ChoiceCountMismatch(usize, usize),
    #[error("number of keys does not match the tape: recorded {0}, got {1}")]
    KeyCountMismatch(usize, usize),
    #[error("receiver's choices are inconsistent")]
    InconsistentChoice,
    #[error("receiver did not send a commitment")]
    NoCommitment,
    #[error(transparent)]
    CommitmentError(#[from] mpz_core::commit::CommitmentError),
    #[error("tape was not recorded")]
    TapeNotRecorded,
}
