use crate::ferret::mpcot::error::SenderError;
use enum_try_as_inner::EnumTryAsInner;
use mpz_core::Block;
use mpz_ot_core::ferret::{
    mpcot::{
        msgs::HashSeed,
        sender::{state, Sender as SenderCore},
    },
    spcot::sender::Sender as SpcotSender,
};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    PreExtension(SenderCore<state::PreExtension>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// MPCOT sender.
#[derive(Debug)]
pub struct Sender {
    state: State,
    spcot: SpcotSender,
}

impl Sender {
    /// Creates a new Sender.
    ///
    /// # Arguments
    ///
    /// * `spcot` - A spcot sender.
    pub fn new(spcot: SpcotSender) -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            spcot,
        }
    }

    /// Performs setup with the provided delta.
    ///
    /// # Arguments
    ///
    /// * `delta` - The delta value to use for OT extension.
    /// * `hash_seed` - The seed for Cuckoo hash sent by the receiver.
    pub fn setup_with_delta(
        &mut self,
        delta: Block,
        hash_seed: HashSeed,
    ) -> Result<(), SenderError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_sender = ext_sender.setup(delta, hash_seed);

        self.state = State::PreExtension(ext_sender);

        Ok(())
    }
}
