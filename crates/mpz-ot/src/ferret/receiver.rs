use crate::{
    ferret::{mpcot::Receiver as MpcotReceiver, ReceiverError},
    RandomCOTReceiver,
};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::{prg::Prg, Block};
use mpz_ot_core::{
    ferret::receiver::{state, Receiver as ReceiverCore},
    RCOTReceiverOutput,
};
use serio::SinkExt;
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

use super::FerretConfig;
use crate::{async_trait, OTError};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(ReceiverCore<state::Initialized>),
    Extension(ReceiverCore<state::Extension>),
    Complete,
    Error,
}

/// Ferret Receiver.
#[derive(Debug)]
pub struct Receiver<RandomCOT, SetupRandomCOT> {
    state: State,
    mpcot: MpcotReceiver<RandomCOT>,
    config: FerretConfig<RandomCOT, SetupRandomCOT>,
}

impl<RandomCOT, SetupRandomCOT> Receiver<RandomCOT, SetupRandomCOT>
where
    RandomCOT: Send + Default + Clone,
    SetupRandomCOT: Send,
{
    /// Creates a new Receiver.
    ///
    /// # Arguments.
    ///
    /// * `config` - Ferret configuration.
    pub fn new(config: FerretConfig<RandomCOT, SetupRandomCOT>) -> Self {
        Self {
            state: State::Initialized(ReceiverCore::new()),
            mpcot: MpcotReceiver::new(config.lpn_type()),
            config,
        }
    }

    /// Setup for receiver.
    ///
    /// # Arguments.
    ///
    /// * `ctx` - The channel context.
    pub async fn setup<Ctx>(&mut self, ctx: &mut Ctx) -> Result<(), ReceiverError>
    where
        Ctx: Context,
        SetupRandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let rcot = self.config.rcot();
        self.mpcot.setup(ctx, rcot).await?;

        let params = self.config.lpn_parameters();
        let lpn_type = self.config.lpn_type();

        // Get random blocks from ideal Random COT.

        let RCOTReceiverOutput {
            choices: u,
            msgs: w,
            ..
        } = self
            .config
            .setup_rcot()
            .receive_random_correlated(ctx, params.k)
            .await?;

        let seed = Prg::new().random_block();

        let (ext_receiver, seed) = ext_receiver.setup(params, lpn_type, seed, &u, &w)?;

        ctx.io_mut().send(seed).await?;

        self.state = State::Extension(ext_receiver);

        Ok(())
    }

    /// Performs extension.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The channel context.
    async fn extend<Ctx>(&mut self, ctx: &mut Ctx) -> Result<(Vec<bool>, Vec<Block>), ReceiverError>
    where
        Ctx: Context,
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let mut ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let (alphas, n) = ext_receiver.get_mpcot_query();

        let r = self.mpcot.extend(ctx, &alphas, n as u32).await?;

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
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;
        self.state = State::Complete;
        self.mpcot.finalize()?;

        Ok(())
    }
}

#[async_trait]
impl<Ctx, RandomCOT, SetupRandomCOT> RandomCOTReceiver<Ctx, bool, Block>
    for Receiver<RandomCOT, SetupRandomCOT>
where
    Ctx: Context,
    RandomCOT: RandomCOTReceiver<Ctx, bool, Block> + Send + Clone + Default + 'static,
    SetupRandomCOT: Send + 'static,
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
