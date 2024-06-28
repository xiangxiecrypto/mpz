use std::mem;

use async_trait::async_trait;
use enum_try_as_inner::EnumTryAsInner;
use futures::TryFutureExt;
use itybity::IntoBits;
use mpz_cointoss as cointoss;
use mpz_common::{try_join, Allocate, Context, Preprocess};
use mpz_core::{prg::Prg, Block};
use mpz_ot_core::{
    kos::{
        extension_matrix_size,
        msgs::{Extend, StartExtend},
        pad_ot_count, sender_state as state, Sender as SenderCore, SenderConfig, SenderKeys, CSP,
    },
    OTSenderOutput, ROTSenderOutput,
};
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use rand_core::SeedableRng;
use serio::{stream::IoStreamExt as _, SinkExt as _};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

use crate::{
    kos::SenderError, CommittedOTReceiver, CommittedOTSender, OTError, OTReceiver, OTSender,
    OTSetup, RandomOTSender,
};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// KOS sender.
#[derive(Debug)]
pub struct Sender<BaseOT> {
    state: State,
    base: BaseOT,
    alloc: usize,
    cointoss_sender: Option<cointoss::Sender<cointoss::sender_state::Received>>,
}

impl<BaseOT: Send> Sender<BaseOT> {
    /// Creates a new Sender
    ///
    /// # Arguments
    ///
    /// * `config` - The Sender's configuration
    pub fn new(config: SenderConfig, base: BaseOT) -> Self {
        Self {
            state: State::Initialized(SenderCore::new(config)),
            base,
            alloc: 0,
            cointoss_sender: None,
        }
    }

    /// The number of remaining OTs which can be consumed.
    pub fn remaining(&self) -> Result<usize, SenderError> {
        Ok(self.state.try_as_extension()?.remaining())
    }

    /// Returns the provided number of keys.
    pub(crate) fn take_keys(&mut self, count: usize) -> Result<SenderKeys, SenderError> {
        self.state
            .try_as_extension_mut()?
            .keys(count)
            .map_err(SenderError::from)
    }

    /// Performs the base OT setup with the provided delta.
    ///
    /// # Arguments
    ///
    /// * `sink` - The sink to send messages to the base OT sender
    /// * `stream` - The stream to receive messages from the base OT sender
    /// * `delta` - The delta value to use for the base OT setup.
    pub async fn setup_with_delta<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        delta: Block,
    ) -> Result<(), SenderError>
    where
        BaseOT: OTReceiver<Ctx, bool, Block>,
    {
        if self.state.try_as_initialized()?.config().sender_commit() {
            return Err(SenderError::ConfigError(
                "committed sender can not choose delta".to_string(),
            ));
        }

        self._setup_with_delta(ctx, delta).await
    }

    async fn _setup_with_delta<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        delta: Block,
    ) -> Result<(), SenderError>
    where
        BaseOT: OTReceiver<Ctx, bool, Block>,
    {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let choices = delta.into_lsb0_vec();
        let base_output = self.base.receive(ctx, &choices).await?;

        let seeds: [Block; CSP] = base_output
            .msgs
            .try_into()
            .expect("seeds should be CSP length");

        let ext_sender = ext_sender.setup(delta, seeds);

        self.state = State::Extension(ext_sender);

        Ok(())
    }

    /// Performs OT extension.
    ///
    /// # Arguments
    ///
    /// * `channel` - The channel to communicate with the receiver.
    /// * `count` - The number of OTs to extend.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<(), SenderError> {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let count = pad_ot_count(count);

        let StartExtend {
            count: receiver_count,
        } = ctx.io_mut().expect_next().await?;

        if count != receiver_count {
            return Err(SenderError::ConfigError(
                "sender and receiver count mismatch".to_string(),
            ));
        }

        let expected_us = extension_matrix_size(count);
        let mut extend = Extend {
            us: Vec::with_capacity(expected_us),
        };

        // Receive extension matrix from the receiver.
        while extend.us.len() < expected_us {
            let Extend { us: chunk } = ctx.io_mut().expect_next().await?;

            extend.us.extend(chunk);
        }

        // Extend the OTs.
        let mut ext_sender =
            Backend::spawn(move || ext_sender.extend(count, extend).map(|_| ext_sender)).await?;

        // Sample chi_seed with coin-toss.
        let seed: Block = thread_rng().gen();
        let chi_seed = cointoss::cointoss_receiver(ctx, vec![seed]).await?[0];

        // Receive the receiver's check.
        let receiver_check = ctx.io_mut().expect_next().await?;

        // Check consistency of extension.
        let ext_sender = Backend::spawn(move || {
            ext_sender
                .check(chi_seed, receiver_check)
                .map(|_| ext_sender)
        })
        .await?;

        self.state = State::Extension(ext_sender);

        Ok(())
    }
}

impl<BaseOT: Send> Sender<BaseOT> {
    pub(crate) async fn reveal<Ctx: Context>(&mut self, ctx: &mut Ctx) -> Result<(), SenderError>
    where
        BaseOT: CommittedOTReceiver<Ctx, bool, Block>,
    {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        // Reveal coin toss payload
        let Some(sender) = self.cointoss_sender.take() else {
            return Err(SenderError::ConfigError(
                "committed sender not configured".to_string(),
            ))?;
        };

        sender.finalize(ctx).await.map_err(SenderError::from)?;

        // Reveal base OT choices
        self.base.reveal_choices(ctx).await?;

        // This sender is no longer usable, so mark it as complete.
        self.state = State::Complete;

        Ok(())
    }
}

#[async_trait]
impl<Ctx, BaseOT> OTSetup<Ctx> for Sender<BaseOT>
where
    Ctx: Context,
    BaseOT: OTSetup<Ctx> + OTReceiver<Ctx, bool, Block> + Send + 'static,
{
    async fn setup(&mut self, ctx: &mut Ctx) -> Result<(), OTError> {
        if self.state.is_extension() {
            return Ok(());
        }

        let sender = std::mem::replace(&mut self.state, State::Error)
            .try_into_initialized()
            .map_err(SenderError::from)?;

        // If the sender is committed, we sample delta using a coin toss.
        let delta = if sender.config().sender_commit() {
            let cointoss_seed = thread_rng().gen();

            // Execute coin-toss protocol and base OT setup concurrently.
            let ((seeds, cointoss_sender), _) = try_join!(
                ctx,
                async {
                    cointoss::Sender::new(vec![cointoss_seed])
                        .commit(ctx)
                        .await?
                        .receive(ctx)
                        .await
                        .map_err(SenderError::from)
                },
                self.base.setup(ctx).map_err(SenderError::from)
            )??;

            // Store the sender to finalize the cointoss protocol later.
            self.cointoss_sender = Some(cointoss_sender);

            seeds[0]
        } else {
            self.base.setup(ctx).await?;
            Block::random(&mut thread_rng())
        };

        self.state = State::Initialized(sender);

        self._setup_with_delta(ctx, delta)
            .await
            .map_err(OTError::from)
    }
}

impl<BaseOT> Allocate for Sender<BaseOT> {
    fn alloc(&mut self, count: usize) {
        self.alloc += count;
    }
}

#[async_trait]
impl<Ctx, BaseOT> Preprocess<Ctx> for Sender<BaseOT>
where
    Ctx: Context,
    BaseOT: OTSetup<Ctx> + OTReceiver<Ctx, bool, Block> + Send + 'static,
{
    type Error = OTError;

    async fn preprocess(&mut self, ctx: &mut Ctx) -> Result<(), OTError> {
        if self.state.is_initialized() {
            self.setup(ctx).await?;
        }

        let count = mem::take(&mut self.alloc);
        if count == 0 {
            return Ok(());
        }

        self.extend(ctx, count).await.map_err(OTError::from)
    }
}

#[async_trait]
impl<Ctx, BaseOT> OTSender<Ctx, [Block; 2]> for Sender<BaseOT>
where
    Ctx: Context,
    BaseOT: Send,
{
    async fn send(
        &mut self,
        ctx: &mut Ctx,
        msgs: &[[Block; 2]],
    ) -> Result<OTSenderOutput, OTError> {
        let sender = self
            .state
            .try_as_extension_mut()
            .map_err(SenderError::from)?;

        let derandomize = ctx.io_mut().expect_next().await?;

        let mut sender_keys = sender.keys(msgs.len()).map_err(SenderError::from)?;
        sender_keys
            .derandomize(derandomize)
            .map_err(SenderError::from)?;
        let payload = sender_keys
            .encrypt_blocks(msgs)
            .map_err(SenderError::from)?;
        let id = payload.id;

        ctx.io_mut()
            .send(payload)
            .await
            .map_err(SenderError::from)?;

        Ok(OTSenderOutput { id })
    }
}

#[async_trait]
impl<Ctx, const N: usize, BaseOT> OTSender<Ctx, [[u8; N]; 2]> for Sender<BaseOT>
where
    Ctx: Context,
    BaseOT: Send,
{
    async fn send(
        &mut self,
        ctx: &mut Ctx,
        msgs: &[[[u8; N]; 2]],
    ) -> Result<OTSenderOutput, OTError> {
        let sender = self
            .state
            .try_as_extension_mut()
            .map_err(SenderError::from)?;

        let derandomize = ctx.io_mut().expect_next().await?;

        let mut sender_keys = sender.keys(msgs.len()).map_err(SenderError::from)?;
        sender_keys
            .derandomize(derandomize)
            .map_err(SenderError::from)?;
        let payload = sender_keys.encrypt_bytes(msgs).map_err(SenderError::from)?;
        let id = payload.id;

        ctx.io_mut()
            .send(payload)
            .await
            .map_err(SenderError::from)?;

        Ok(OTSenderOutput { id })
    }
}

#[async_trait]
impl<Ctx, T, BaseOT> RandomOTSender<Ctx, [T; 2]> for Sender<BaseOT>
where
    Ctx: Context,
    Standard: Distribution<T>,
    BaseOT: Send,
{
    async fn send_random(
        &mut self,
        _ctx: &mut Ctx,
        count: usize,
    ) -> Result<ROTSenderOutput<[T; 2]>, OTError> {
        let sender = self
            .state
            .try_as_extension_mut()
            .map_err(SenderError::from)?;

        let keys = sender.keys(count).map_err(SenderError::from)?;
        let id = keys.id();

        let msgs = keys
            .take_keys()
            .into_iter()
            .map(|[k0, k1]| {
                let mut prg_0 = Prg::from_seed(k0);
                let mut prg_1 = Prg::from_seed(k1);

                [prg_0.gen::<T>(), prg_1.gen::<T>()]
            })
            .collect();

        Ok(ROTSenderOutput { id, msgs })
    }
}

#[async_trait]
impl<Ctx, BaseOT> CommittedOTSender<Ctx, [Block; 2]> for Sender<BaseOT>
where
    Ctx: Context,
    BaseOT: CommittedOTReceiver<Ctx, bool, Block> + Send,
{
    async fn reveal(&mut self, ctx: &mut Ctx) -> Result<(), OTError> {
        self.reveal(ctx).await.map_err(OTError::from)
    }
}
