//! Implementation of VOPE receiver.

use crate::vope::error::ReceiverError;
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot::{RCOTReceiverOutput, RandomCOTReceiver};
use mpz_zk_core::vope::{
    receiver::{state, Receiver as ReceiverCore},
    CSP,
};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
#[allow(missing_docs)]
pub enum State {
    Initialized(ReceiverCore<state::Initialized>),
    Extension(ReceiverCore<state::Extension>),
    Complete,
    Error,
}

/// VOPE receiver (prover)
#[derive(Debug)]
pub struct Receiver<RandomCOT> {
    state: State,
    rcot: RandomCOT,
}

impl<RandomCOT: Send> Receiver<RandomCOT> {
    /// Creates a new receiver.
    ///
    /// # Arguments
    ///
    /// * `rcot` - The random COT used by the receiver.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(ReceiverCore::new()),
            rcot,
        }
    }

    /// Performs setup for receiver.
    pub fn setup(&mut self) -> Result<(), ReceiverError> {
        let ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_receiver = ext_receiver.setup();

        self.state = State::Extension(ext_receiver);

        Ok(())
    }

    /// Performs VOPE extension for receiver.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `d` - The polynomial degree.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        d: usize,
    ) -> Result<Vec<Block>, ReceiverError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let mut ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        assert!(d > 0);

        let RCOTReceiverOutput {
            msgs: ms,
            choices: us,
            ..
        } = self
            .rcot
            .receive_random_correlated(ctx, (2 * d - 1) * CSP)
            .await?;

        // extend
        let (ext_receiver, res) = Backend::spawn(move || {
            ext_receiver
                .extend(&ms, &us, d)
                .map(|res| (ext_receiver, res))
        })
        .await?;

        self.state = State::Extension(ext_receiver);

        Ok(res)
    }

    /// Complete extension.
    pub fn finalize(&mut self) -> Result<(), ReceiverError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.state = State::Complete;

        Ok(())
    }
}
