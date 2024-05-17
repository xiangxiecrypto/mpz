use crate::{
    ferret::{mpcot::error::SenderRegularError, spcot::Sender as SpcotSender},
    RandomCOTSender,
};
use enum_try_as_inner::EnumTryAsInner;

use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::ferret::mpcot::sender_regular::{state, Sender as SenderCore};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

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
pub struct SenderRegular<RandomCOT> {
    state: State,
    spcot: SpcotSender<RandomCOT>,
}

impl<RandomCOT: Send> SenderRegular<RandomCOT> {
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
    /// # Argument
    ///
    /// `delta` - The delta value to use for OT extension.
    pub fn setup_with_delta(&mut self, delta: Block) -> Result<(), SenderRegularError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_sender = ext_sender.setup(delta);

        self.state = State::Extension(ext_sender);
        self.spcot.setup_with_delta(delta)?;

        Ok(())
    }

    /// Performs MPCOT regular extension.
    ///
    /// # Argument
    ///
    /// * `ctx` - The context.
    /// * `t` - The number of queried indices.
    /// * `n` - The total number of indices.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        t: u32,
        n: u32,
    ) -> Result<Vec<Block>, SenderRegularError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        println!("here");
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let (ext_sender, hs) = Backend::spawn(move || ext_sender.pre_extend(t, n)).await?;

        self.spcot.extend(ctx, &hs).await?;

        let st = self.spcot.check(ctx).await?;

        let (ext_sender, output) = Backend::spawn(move || ext_sender.extend(&st)).await?;

        self.state = State::Extension(ext_sender);

        Ok(output)
    }

    /// Compete extension.
    pub fn finalize(&mut self) -> Result<(), SenderRegularError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.spcot.finalize()?;
        self.state = State::Complete;

        Ok(())
    }
}
