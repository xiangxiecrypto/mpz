use crate::{ferret::spcot::error::ReceiverError, RandomCOTReceiver};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::{
    ferret::{
        spcot::receiver::{state, Receiver as ReceiverCore},
        CSP,
    },
    RCOTReceiverOutput,
};
use serio::{stream::IoStreamExt, SinkExt};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(ReceiverCore<state::Initialized>),
    Extension(Box<ReceiverCore<state::Extension>>),
    Complete,
    Error,
}

/// SPCOT Receiver.
#[derive(Debug)]
pub struct Receiver<RandomCOT> {
    state: State,
    rcot: RandomCOT,
}

impl<RandomCOT: Send> Receiver<RandomCOT> {
    /// Creates a new Receiver.
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
        self.state = State::Extension(Box::new(ext_receiver));
        Ok(())
    }

    /// Performs spcot extension for receiver.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `alpha`` - The chosen position.
    /// * `h` - The depth of GGM tree.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        alpha: u32,
        h: usize,
    ) -> Result<(), ReceiverError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let mut ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let RCOTReceiverOutput {
            choices: rs,
            msgs: ts,
            ..
        } = self.rcot.receive_random_correlated(ctx, h).await?;

        // extend
        let (mut ext_receiver, mask) = Backend::spawn(move || {
            ext_receiver
                .extend_mask_bits(h, alpha, &rs)
                .map(|mask| (ext_receiver, mask))
        })
        .await?;

        ctx.io_mut().send(mask).await?;

        let extendfs = ctx.io_mut().expect_next().await?;

        let ext_receiver = Backend::spawn(move || {
            ext_receiver
                .extend(h, alpha, &ts, extendfs)
                .map(|_| ext_receiver)
        })
        .await?;

        self.state = State::Extension(ext_receiver);

        Ok(())
    }

    /// Performs batch check for SPCOT extension.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    pub async fn check<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
    ) -> Result<Vec<(Vec<Block>, u32)>, ReceiverError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let mut ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        // batch check
        let RCOTReceiverOutput {
            choices: x_star,
            msgs: z_star,
            ..
        } = self.rcot.receive_random_correlated(ctx, CSP).await?;

        let (mut ext_receiver, checkfr) = Backend::spawn(move || {
            ext_receiver
                .check_pre(&x_star)
                .map(|checkfr| (ext_receiver, checkfr))
        })
        .await?;

        ctx.io_mut().send(checkfr).await?;
        let check = ctx.io_mut().expect_next().await?;

        let output = Backend::spawn(move || ext_receiver.check(&z_star, check)).await?;

        self.state = State::Complete;

        Ok(output)
    }
}
