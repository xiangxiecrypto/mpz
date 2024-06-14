use crate::{ferret::spcot::error::ReceiverError, RandomCOTReceiver};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::{
    ferret::{
        spcot::{
            msgs::ExtendFromSender,
            receiver::{state, Receiver as ReceiverCore},
        },
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
pub(crate) struct Receiver<RandomCOT> {
    state: State,
    rcot: RandomCOT,
}

impl<RandomCOT: Send + Default> Receiver<RandomCOT> {
    /// Creates a new Receiver.
    pub(crate) fn new() -> Self {
        Self {
            state: State::Initialized(ReceiverCore::new()),
            rcot: Default::default(),
        }
    }

    /// Performs setup for receiver.
    ///
    /// # Arguments.
    ///
    /// * `rcot` - The random COT used by the receiver.
    pub(crate) fn setup(&mut self, rcot: RandomCOT) -> Result<(), ReceiverError> {
        let ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_receiver = ext_receiver.setup();
        self.state = State::Extension(Box::new(ext_receiver));
        self.rcot = rcot;
        Ok(())
    }

    /// Performs spcot extension for receiver.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `alphas`` - The vector of chosen positions.
    /// * `h` - The depth of GGM tree.
    pub(crate) async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        alphas: &[u32],
        hs: &[usize],
    ) -> Result<(), ReceiverError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let mut ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let h = hs.iter().sum();
        let RCOTReceiverOutput {
            choices: rss,
            msgs: tss,
            ..
        } = self.rcot.receive_random_correlated(ctx, h).await?;

        // extend
        let h_in = hs.to_vec();
        let alphas_in = alphas.to_vec();
        let (mut ext_receiver, masks) = Backend::spawn(move || {
            ext_receiver
                .extend_mask_bits(&h_in, &alphas_in, &rss)
                .map(|mask| (ext_receiver, mask))
        })
        .await?;

        ctx.io_mut().send(masks).await?;

        let extendfss: Vec<ExtendFromSender> = ctx.io_mut().expect_next().await?;

        let h_in = hs.to_vec();
        let alphas_in = alphas.to_vec();
        let ext_receiver = Backend::spawn(move || {
            ext_receiver
                .extend(&h_in, &alphas_in, &tss, &extendfss)
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
    pub(crate) async fn check<Ctx: Context>(
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

        let (ext_receiver, output) = Backend::spawn(move || {
            ext_receiver
                .check(&z_star, check)
                .map(|output| (ext_receiver, output))
        })
        .await?;

        self.state = State::Extension(ext_receiver);

        Ok(output)
    }

    /// Complete extension.
    pub(crate) fn finalize(&mut self) -> Result<(), ReceiverError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.state = State::Complete;

        Ok(())
    }
}
