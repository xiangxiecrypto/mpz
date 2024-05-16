use crate::ferret::{mpcot::error::SenderRegularError, spcot::Sender as SpcotSender};
use enum_try_as_inner::EnumTryAsInner;

use mpz_core::Block;
use mpz_ot_core::ferret::mpcot::sender_regular::{state, Sender as SenderCore};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// MPCOT regular sender.
#[derive(Debug)]
pub struct Sender<RandomCOT> {
    state: State,
    spcot: SpcotSender<RandomCOT>,
}

impl<RandomCOT: Send> Sender<RandomCOT> {
    /// Creates a new Sender.
    ///
    /// # Arguments
    ///
    /// * `rcot` - A rcot sender.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            spcot: crate::ferret::spcot::Sender::new(rcot),
        }
    }

    /// Performs setup with the provided delta.
    ///
    /// # Arguments
    ///
    /// `delta` - The delta value to use for OT extension.
    pub fn setup_with_delta(&mut self, delta: Block) -> Result<(), SenderRegularError> {
        // let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;
        Ok(())
    }
}
