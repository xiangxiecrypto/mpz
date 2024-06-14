use crate::{ferret::spcot::error::SenderError, RandomCOTSender};
use enum_try_as_inner::EnumTryAsInner;
use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::{
    ferret::{
        spcot::{
            msgs::MaskBits,
            sender::{state, Sender as SenderCore},
        },
        CSP,
    },
    RCOTSenderOutput,
};
use serio::{stream::IoStreamExt, SinkExt};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(Box<SenderCore<state::Extension>>),
    Complete,
    Error,
}

/// SPCOT sender.
#[derive(Debug)]
pub(crate) struct Sender<RandomCOT> {
    state: State,
    rcot: RandomCOT,
}

impl<RandomCOT: Send + Default> Sender<RandomCOT> {
    /// Creates a new Sender.
    pub(crate) fn new() -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            rcot: Default::default(),
        }
    }

    /// Performs setup with the provided delta.
    ///
    /// # Arguments
    ///
    /// * `delta` - The delta value to use for OT extension.
    /// * `rcot` - The random COT used by the sender.
    pub(crate) fn setup_with_delta(
        &mut self,
        delta: Block,
        rcot: RandomCOT,
    ) -> Result<(), SenderError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_sender = ext_sender.setup(delta);

        self.state = State::Extension(Box::new(ext_sender));
        self.rcot = rcot;
        Ok(())
    }

    /// Performs spcot extension for sender.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `hs` - The depths of GGM trees.
    pub(crate) async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        hs: &[usize],
    ) -> Result<(), SenderError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let h = hs.iter().sum();
        let RCOTSenderOutput { msgs: qss, .. } = self.rcot.send_random_correlated(ctx, h).await?;

        let masks: Vec<MaskBits> = ctx.io_mut().expect_next().await?;

        // extend
        let h_in = hs.to_vec();
        let (ext_sender, extend_msg) = Backend::spawn(move || {
            ext_sender
                .extend(&h_in, &qss, &masks)
                .map(|extend_msg| (ext_sender, extend_msg))
        })
        .await?;

        ctx.io_mut().send(extend_msg).await?;

        self.state = State::Extension(ext_sender);

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
    ) -> Result<Vec<Vec<Block>>, SenderError>
    where
        RandomCOT: RandomCOTSender<Ctx, Block>,
    {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        // batch check
        let RCOTSenderOutput { msgs: y_star, .. } =
            self.rcot.send_random_correlated(ctx, CSP).await?;

        let checkfr = ctx.io_mut().expect_next().await?;

        let (ext_sender, output, check_msg) = Backend::spawn(move || {
            ext_sender
                .check(&y_star, checkfr)
                .map(|(output, check_msg)| (ext_sender, output, check_msg))
        })
        .await?;

        ctx.io_mut().send(check_msg).await?;

        self.state = State::Extension(ext_sender);

        Ok(output)
    }

    /// Complete extension.
    pub(crate) fn finalize(&mut self) -> Result<(), SenderError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.state = State::Complete;

        Ok(())
    }
}
