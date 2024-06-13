//! Implementation of VOPE sender

use crate::vope::error::SenderError;
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot::{RCOTSenderOutput, RandomCOTSender};
use mpz_zk_core::vope::{
    sender::{state, Sender as SenderCore},
    CSP,
};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
#[allow(missing_docs)]
pub enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// VOPE sender (verifier)
#[derive(Debug)]
pub struct Sender<RandomCOT> {
    state: State,
    rcot: RandomCOT,
}

impl<RandomCOT: Send> Sender<RandomCOT> {
    /// Creates a new Sender.
    ///
    /// # Arguments
    ///
    /// * `rcot` - The random COT used by the sender.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            rcot,
        }
    }

    /// Performs setup with the provided delta.
    ///
    /// # Arguments
    ///
    /// * `delta` - The delta value to use for VOPE extension.
    pub fn setup_with_delta(&mut self, delta: Block) -> Result<(), SenderError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_sender = ext_sender.setup(delta);

        self.state = State::Extension(ext_sender);

        Ok(())
    }

    /// Performs VOPE extension for sender.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `d` - The polynomial degree.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        d: usize,
    ) -> Result<Block, SenderError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        assert!(d > 0);

        let RCOTSenderOutput { msgs: ks, .. } = self
            .rcot
            .send_random_correlated(ctx, (2 * d - 1) * CSP)
            .await?;

        let (ext_sender, res) =
            Backend::spawn(move || ext_sender.extend(&ks, d).map(|res| (ext_sender, res))).await?;

        self.state = State::Extension(ext_sender);

        Ok(res)
    }

    /// Complete extension.
    pub fn finalize(&mut self) -> Result<(), SenderError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.state = State::Complete;

        Ok(())
    }
}
