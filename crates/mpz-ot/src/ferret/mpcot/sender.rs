use crate::{
    ferret::{mpcot::error::SenderError, spcot::Sender as SpcotSender},
    RandomCOTSender,
};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::ferret::{
    mpcot::{
        msgs::HashSeed,
        sender::{state as uniform_state, Sender as UniformSenderCore},
        sender_regular::{state as regular_state, Sender as RegularSenderCore},
    },
    LpnType,
};
use serio::stream::IoStreamExt;
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    UniformInitialized(UniformSenderCore<uniform_state::Initialized>),
    UniformExtension(UniformSenderCore<uniform_state::Extension>),
    RegularInitialized(RegularSenderCore<regular_state::Initialized>),
    RegularExtension(RegularSenderCore<regular_state::Extension>),
    Complete,
    Error,
}

/// MPCOT sender.
#[derive(Debug)]
pub(crate) struct Sender<RandomCOT> {
    state: State,
    spcot: SpcotSender<RandomCOT>,
    lpn_type: LpnType,
}

impl<RandomCOT: Send + Default> Sender<RandomCOT> {
    /// Creates a new Sender.
    ///
    /// # Arguments.
    ///
    /// * `lpn_type` - The type of LPN.
    pub(crate) fn new(lpn_type: LpnType) -> Self {
        match lpn_type {
            LpnType::Uniform => Self {
                state: State::UniformInitialized(UniformSenderCore::new()),
                spcot: crate::ferret::spcot::Sender::new(),
                lpn_type,
            },
            LpnType::Regular => Self {
                state: State::RegularInitialized(RegularSenderCore::new()),
                spcot: crate::ferret::spcot::Sender::new(),
                lpn_type,
            },
        }
    }

    /// Performs setup with provided delta.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The channel.
    /// * `delta` - The delta value to use for OT extension.
    /// * `rcot` - The random COT used by Sender.
    pub(crate) async fn setup_with_delta<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        delta: Block,
        rcot: RandomCOT,
    ) -> Result<(), SenderError> {
        match self.lpn_type {
            LpnType::Uniform => {
                let ext_sender = std::mem::replace(&mut self.state, State::Error)
                    .try_into_uniform_initialized()?;

                let hash_seed: HashSeed = ctx.io_mut().expect_next().await?;

                let ext_sender = ext_sender.setup(delta, hash_seed);

                self.state = State::UniformExtension(ext_sender);
            }

            LpnType::Regular => {
                let ext_sender = std::mem::replace(&mut self.state, State::Error)
                    .try_into_regular_initialized()?;

                let ext_sender = ext_sender.setup(delta);

                self.state = State::RegularExtension(ext_sender);
            }
        }

        self.spcot.setup_with_delta(delta, rcot)?;

        Ok(())
    }

    /// Performs MPCOT extension.
    ///
    ///
    /// # Arguments.
    ///
    /// * `ctx` - The context.
    /// * `t` - The number of queried indices.
    /// * `n` - The total number of indices.
    pub(crate) async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        t: u32,
        n: u32,
    ) -> Result<Vec<Block>, SenderError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        match self.lpn_type {
            LpnType::Uniform => {
                let ext_sender = std::mem::replace(&mut self.state, State::Error)
                    .try_into_uniform_extension()?;

                let (ext_sender, hs) = Backend::spawn(move || ext_sender.pre_extend(t, n)).await?;

                self.spcot.extend(ctx, &hs).await?;

                let st = self.spcot.check(ctx).await?;

                let (ext_sender, output) = Backend::spawn(move || ext_sender.extend(&st)).await?;

                self.state = State::UniformExtension(ext_sender);
                Ok(output)
            }
            LpnType::Regular => {
                let ext_sender = std::mem::replace(&mut self.state, State::Error)
                    .try_into_regular_extension()?;

                let (ext_sender, hs) = Backend::spawn(move || ext_sender.pre_extend(t, n)).await?;

                self.spcot.extend(ctx, &hs).await?;

                let st = self.spcot.check(ctx).await?;

                let (ext_sender, output) = Backend::spawn(move || ext_sender.extend(&st)).await?;

                self.state = State::RegularExtension(ext_sender);
                Ok(output)
            }
        }
    }

    /// Complete extension.
    pub(crate) fn finalize(&mut self) -> Result<(), SenderError> {
        match self.lpn_type {
            LpnType::Uniform => {
                std::mem::replace(&mut self.state, State::Error).try_into_uniform_extension()?;
            }
            LpnType::Regular => {
                std::mem::replace(&mut self.state, State::Error).try_into_regular_extension()?;
            }
        }

        self.spcot.finalize()?;
        self.state = State::Complete;

        Ok(())
    }
}
