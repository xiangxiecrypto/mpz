use crate::ferret::ReceiverError;
use crate::OTError;
use crate::{
    ferret::mpcot::{Receiver as MpcotUniformReceiver, ReceiverRegular as MpcotRegularReceiver},
    RandomCOTReceiver,
};
use async_trait::async_trait;
use enum_try_as_inner::EnumTryAsInner;

use mpz_common::Context;
use mpz_core::prg::Prg;
use mpz_core::{lpn::LpnParameters, Block};
use mpz_ot_core::ferret::{
    receiver::{state, Receiver as ReceiverCore},
    LpnType,
};
use mpz_ot_core::RCOTReceiverOutput;
use serio::SinkExt;
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(ReceiverCore<state::Initialized>),
    Extension(ReceiverCore<state::Extension>),
    Complete,
    Error,
}

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum MpcotReceiver<RandomCOT> {
    Uniform(MpcotUniformReceiver<RandomCOT>),
    Regular(MpcotRegularReceiver<RandomCOT>),
    Initial(RandomCOT),
    Error,
}

/// Ferret receiver.
#[derive(Debug)]
pub struct Receiver<RandomCOT> {
    state: State,
    mpcot: MpcotReceiver<RandomCOT>,
}

impl<RandomCOT: Send> Receiver<RandomCOT> {
    /// Creates a new receiver.
    ///
    /// # Argument
    ///
    /// * `rcot` - A rcot receiver for MPCOT.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(ReceiverCore::new()),
            mpcot: MpcotReceiver::Initial(rcot),
        }
    }

    /// Setup with provided parameters.
    ///
    /// # Argument
    ///
    /// * `ctx` - The channel context.
    /// * `setup_rcot` - A random COT for setup.
    /// * `lpn_parameters` - The LPN parameters for ferret.
    /// * `lpn_type` - The type of lpn problem (general or regular).}
    pub async fn setup_with_parameters<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        setup_rcot: &mut impl RandomCOTReceiver<Ctx, bool, Block>,
        lpn_parameters: LpnParameters,
        lpn_type: LpnType,
    ) -> Result<(), ReceiverError> {
        let ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let rcot = std::mem::replace(&mut self.mpcot, MpcotReceiver::Error).try_into_initial()?;

        // setup mpcot according to lpn_type.
        match lpn_type {
            LpnType::Uniform => {
                let mut mpcot = MpcotUniformReceiver::new(rcot);

                mpcot.setup(ctx).await?;

                self.mpcot = MpcotReceiver::Uniform(mpcot);
            }
            LpnType::Regular => {
                let mut mpcot = MpcotRegularReceiver::new(rcot);

                mpcot.setup()?;

                self.mpcot = MpcotReceiver::Regular(mpcot);
            }
        }

        // Get random blocks from ideal Random COT.

        let RCOTReceiverOutput {
            choices: u,
            msgs: w,
            ..
        } = setup_rcot
            .receive_random_correlated(ctx, lpn_parameters.k)
            .await?;

        let seed = Prg::new().random_block();

        let (ext_receiver, seed) = ext_receiver.setup(lpn_parameters, lpn_type, seed, &u, &w)?;

        ctx.io_mut().send(seed).await?;

        self.state = State::Extension(ext_receiver);

        Ok(())
    }

    /// Performs extension.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The channel context.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
    ) -> Result<(Vec<bool>, Vec<Block>), ReceiverError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let mut ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let (alphas, n) = ext_receiver.get_mpcot_query();

        let mut r = vec![];

        if self.mpcot.is_uniform() {
            let mpcot = self.mpcot.try_as_uniform_mut()?;

            r = mpcot.extend(ctx, &alphas, n as u32).await?;
        } else if self.mpcot.is_regular() {
            let mpcot = self.mpcot.try_as_regular_mut()?;

            r = mpcot.extend(ctx, &alphas, n as u32).await?;
        }

        let (ext_receiver, choices, msgs) = Backend::spawn(move || {
            ext_receiver
                .extend(&r)
                .map(|(choices, msgs)| (ext_receiver, choices, msgs))
        })
        .await?;

        self.state = State::Extension(ext_receiver);

        Ok((choices, msgs))
    }

    /// Complete extension
    pub fn finalize(&mut self) -> Result<(), ReceiverError> {
        self.state = State::Complete;

        if self.mpcot.is_uniform() {
            let mpcot = self.mpcot.try_as_uniform_mut()?;
            mpcot.finalize()?;
        } else if self.mpcot.is_regular() {
            let mpcot = self.mpcot.try_as_regular_mut()?;
            mpcot.finalize()?;
        }

        Ok(())
    }
}

#[async_trait]
impl<Ctx, RandomCOT> RandomCOTReceiver<Ctx, bool, Block> for Receiver<RandomCOT>
where
    Ctx: Context,
    RandomCOT: RandomCOTReceiver<Ctx, bool, Block> + Send + 'static,
{
    async fn receive_random_correlated(
        &mut self,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<RCOTReceiverOutput<bool, Block>, OTError> {
        let (mut choices_buffer, mut msgs_buffer) = self.extend(ctx).await?;

        assert_eq!(choices_buffer.len(), msgs_buffer.len());

        let l = choices_buffer.len();

        let id = self
            .state
            .try_as_extension()
            .map_err(ReceiverError::from)?
            .id();

        if count <= l {
            let choices_res = choices_buffer.drain(..count).collect();

            let msgs_res = msgs_buffer.drain(..count).collect();

            return Ok(RCOTReceiverOutput {
                id,
                choices: choices_res,
                msgs: msgs_res,
            });
        } else {
            let mut choices_res = choices_buffer;
            let mut msgs_res = msgs_buffer;

            for _ in 0..count / l - 1 {
                (choices_buffer, msgs_buffer) = self.extend(ctx).await?;

                choices_res.extend_from_slice(&choices_buffer);
                msgs_res.extend_from_slice(&msgs_buffer);
            }

            (choices_buffer, msgs_buffer) = self.extend(ctx).await?;

            choices_res.extend_from_slice(&choices_buffer[0..count % l]);
            msgs_res.extend_from_slice(&msgs_buffer[0..count % l]);

            return Ok(RCOTReceiverOutput {
                id,
                choices: choices_res,
                msgs: msgs_res,
            });
        }
    }
}
