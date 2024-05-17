use crate::ferret::SenderError;
use crate::OTError;
use crate::{
    ferret::mpcot::{Sender as MpcotUniformSender, SenderRegular as MpcotRegularSender},
    RandomCOTSender,
};
use async_trait::async_trait;
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::{lpn::LpnParameters, Block};
use mpz_ot_core::ferret::{
    sender::{state, Sender as SenderCore},
    LpnType,
};
use mpz_ot_core::RCOTSenderOutput;
use serio::stream::IoStreamExt;

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum MpcotSender<RandomCOT> {
    Uniform(MpcotUniformSender<RandomCOT>),
    Regular(MpcotRegularSender<RandomCOT>),
    Initial(RandomCOT),
    Error,
}

/// Ferret sender.
#[derive(Debug)]
pub struct Sender<RandomCOT> {
    state: State,
    mpcot: MpcotSender<RandomCOT>,
}

impl<RandomCOT: Send> Sender<RandomCOT> {
    /// Creates a new Sender.
    ///
    /// # Argument
    ///
    /// * `rcot` - A rcot sender for MPCOT.
    /// * `setup_rcot` - A rcot sender for setup.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            mpcot: MpcotSender::Initial(rcot),
        }
    }

    /// Setup with provided parameters.
    ///
    /// # Argument
    ///
    /// * `ctx` - The channel context.
    /// * `setup_rcot` - A random COT for setup.
    /// * `delta` - The provided delta used for sender.
    /// * `lpn_parameters` - The LPN parameters for ferret.
    /// * `lpn_type` - The type of lpn problem (general or regular).
    pub async fn setup_with_parameters<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        setup_rcot: &mut impl RandomCOTSender<Ctx, Block>,
        delta: Block,
        lpn_parameters: LpnParameters,
        lpn_type: LpnType,
    ) -> Result<(), SenderError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let rcot = std::mem::replace(&mut self.mpcot, MpcotSender::Error).try_into_initial()?;

        // setup mpcot according to lpn_type
        match lpn_type {
            LpnType::Uniform => {
                let mut mpcot = MpcotUniformSender::new(rcot);

                mpcot.setup_with_delta(ctx, delta).await?;

                self.mpcot = MpcotSender::Uniform(mpcot);
            }
            LpnType::Regular => {
                let mut mpcot = MpcotRegularSender::new(rcot);

                mpcot.setup_with_delta(delta)?;

                self.mpcot = MpcotSender::Regular(mpcot);
            }
        }

        // Get random blocks from ideal Random COT.
        let RCOTSenderOutput { msgs: v, .. } = setup_rcot
            .send_random_correlated(ctx, lpn_parameters.k)
            .await?;

        // Get seed for LPN matrix from receiver.
        let seed = ctx.io_mut().expect_next().await?;

        // Ferret core setup.
        let ext_sender = ext_sender.setup(delta, lpn_parameters, lpn_type, seed, &v)?;

        self.state = State::Extension(ext_sender);

        Ok(())
    }
}

#[async_trait]
impl<Ctx, RandomCOT> RandomCOTSender<Ctx, Block> for Sender<RandomCOT>
where
    Ctx: Context,
    RandomCOT: RandomCOTSender<Ctx, Block> + Send + 'static,
{
    async fn send_random_correlated(
        &mut self,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<RCOTSenderOutput<Block>, OTError> {
        todo!()
    }
}
