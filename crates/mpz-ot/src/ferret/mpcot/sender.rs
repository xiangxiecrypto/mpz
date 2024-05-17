use crate::{
    ferret::{mpcot::error::SenderError, spcot::Sender as SpcotSender},
    RandomCOTSender,
};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::ferret::mpcot::{
    msgs::HashSeed,
    sender::{state, Sender as SenderCore},
};
use serio::stream::IoStreamExt;
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// MPCOT sender.
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
    /// * `delta` - The delta value to use for OT extension.
    /// * `hash_seed` - The seed for Cuckoo hash sent by the receiver.
    pub async fn setup_with_delta<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        delta: Block,
    ) -> Result<(), SenderError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let hash_seed: HashSeed = ctx.io_mut().expect_next().await?;

        let ext_sender = ext_sender.setup(delta, hash_seed);

        self.state = State::Extension(ext_sender);
        self.spcot.setup_with_delta(delta)?;

        Ok(())
    }

    /// Performs MPCOT extension.
    ///
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `t` - The number of queried indices.
    /// * `n` - The total number of indices.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        t: u32,
        n: u32,
    ) -> Result<Vec<Block>, SenderError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let (ext_sender, hs) = Backend::spawn(move || ext_sender.pre_extend(t, n)).await?;

        self.spcot.extend(ctx, &hs).await?;

        let st = self.spcot.check(ctx).await?;

        let (ext_sender, output) = Backend::spawn(move || ext_sender.extend(&st)).await?;

        self.state = State::Extension(ext_sender);

        Ok(output)
    }

    /// Compete extension.
    pub fn finalize(&mut self) -> Result<(), SenderError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.spcot.finalize()?;
        self.state = State::Complete;

        Ok(())
    }
}
