use crate::{ferret::mpcot::Sender as MpcotSender, RandomCOTSender};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::{
    ferret::sender::{state, Sender as SenderCore},
    RCOTSenderOutput,
};
use serio::stream::IoStreamExt;
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

use super::{FerretConfig, SenderError};
use crate::{async_trait, OTError};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// Ferret Sender.
#[derive(Debug)]
pub struct Sender<RandomCOT, SetupRandomCOT> {
    state: State,
    mpcot: MpcotSender<RandomCOT>,
    config: FerretConfig<RandomCOT, SetupRandomCOT>,
}

impl<RandomCOT, SetupRandomCOT> Sender<RandomCOT, SetupRandomCOT>
where
    RandomCOT: Send + Default + Clone,
    SetupRandomCOT: Send,
{
    /// Creates a new Sender.
    pub fn new(config: FerretConfig<RandomCOT, SetupRandomCOT>) -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            mpcot: MpcotSender::new(config.lpn_type()),
            config,
        }
    }

    /// Setup with provided delta.
    ///
    /// # Argument
    ///
    /// * `ctx` - The channel context.
    /// * `delta` - The provided delta used for sender.
    pub async fn setup_with_delta<Ctx>(
        &mut self,
        ctx: &mut Ctx,
        delta: Block,
    ) -> Result<(), SenderError>
    where
        Ctx: Context,
        SetupRandomCOT: RandomCOTSender<Ctx, Block>,
    {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let rcot = self.config.rcot();

        self.mpcot.setup_with_delta(ctx, delta, rcot).await?;

        let params = self.config.lpn_parameters();
        let lpn_type = self.config.lpn_type();

        // Get random blocks from ideal Random COT.
        let RCOTSenderOutput { msgs: v, .. } = self
            .config
            .setup_rcot()
            .send_random_correlated(ctx, params.k)
            .await?;

        // Get seed for LPN matrix from receiver.
        let seed = ctx.io_mut().expect_next().await?;

        // Ferret core setup.
        let ext_sender = ext_sender.setup(delta, params, lpn_type, seed, &v)?;

        self.state = State::Extension(ext_sender);

        Ok(())
    }

    /// Performs extension.
    ///
    /// # Argument
    ///
    /// * `ctx` - The channel context.
    async fn extend<Ctx: Context>(&mut self, ctx: &mut Ctx) -> Result<Vec<Block>, SenderError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let (t, n) = ext_sender.get_mpcot_query();

        let s = self.mpcot.extend(ctx, t, n).await?;

        let (ext_sender, output) =
            Backend::spawn(move || ext_sender.extend(&s).map(|output| (ext_sender, output)))
                .await?;
        self.state = State::Extension(ext_sender);

        Ok(output)
    }

    /// Complete extension
    pub fn finalize(&mut self) -> Result<(), SenderError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;
        self.state = State::Complete;
        self.mpcot.finalize()?;

        Ok(())
    }
}

#[async_trait]
impl<Ctx, RandomCOT, SetupRandomCOT> RandomCOTSender<Ctx, Block>
    for Sender<RandomCOT, SetupRandomCOT>
where
    Ctx: Context,
    RandomCOT: RandomCOTSender<Ctx, Block> + Send + Default + Clone + 'static,
    SetupRandomCOT: Send + 'static,
{
    async fn send_random_correlated(
        &mut self,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<RCOTSenderOutput<Block>, OTError> {
        let mut buffer = self.extend(ctx).await?;
        let l = buffer.len();

        let id = self
            .state
            .try_as_extension()
            .map_err(SenderError::from)?
            .id();

        if count <= l {
            let res = buffer.drain(..count).collect();
            return Ok(RCOTSenderOutput { id, msgs: res });
        } else {
            let mut res = buffer;
            for _ in 0..count / l - 1 {
                buffer = self.extend(ctx).await?;
                res.extend_from_slice(&buffer);
            }

            buffer = self.extend(ctx).await?;
            res.extend_from_slice(&buffer[0..count % l]);

            return Ok(RCOTSenderOutput { id, msgs: res });
        }
    }
}
