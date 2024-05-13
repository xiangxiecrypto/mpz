use crate::{ferret::spcot::error::SenderError, COTSender};
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
    COTSenderOutput,
};
use serio::{stream::IoStreamExt, SinkExt};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(SenderCore<state::Initialized>),
    Extension(SenderCore<state::Extension>),
    Complete,
    Error,
}

/// SPCOT sender.
#[derive(Debug)]
pub struct Sender<RandomCOT> {
    state: State,
    rcot: RandomCOT,
}

impl<RandomCOT: Send> Sender<RandomCOT> {
    /// Creates a new Sender.
    ///
    /// # Arguments
    ///
    /// * `rcot` - The random COT used by the Sender.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(SenderCore::new()),
            rcot,
        }
    }

    /// Performs setup with the provided delta.
    ///
    /// # Arguments
    ///
    /// * `delta` - The delta value to use for OT extension.
    /// * `seed` - The random seed to generate PRG
    pub fn setup_with_delta(&mut self, delta: Block, seed: Block) -> Result<(), SenderError> {
        let ext_sender = std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_sender = ext_sender.setup(delta, seed);

        self.state = State::Extension(ext_sender);
        Ok(())
    }

    /// Performs spcot extension for sender.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `count` - The number of SPCOTs to extend.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<(), SenderError>
    where
        RandomCOT: COTSender<Ctx, Block>,
    {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let h = count
            .checked_next_power_of_two()
            .expect("len should be less than usize::MAX / 2 - 1")
            .ilog2() as usize;

        let COTSenderOutput { msgs: qs, .. } = self.rcot.send_correlated(ctx, count).await?;

        println!("sender reaches here");

        let mask: MaskBits = ctx.io_mut().expect_next().await?;

        // extend
        let (ext_sender, extend_msg) = Backend::spawn(move || {
            ext_sender
                .extend(h, &qs, mask)
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
    pub async fn check<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
    ) -> Result<Vec<Vec<Block>>, SenderError>
    where
        RandomCOT: COTSender<Ctx, Block>,
    {
        let mut ext_sender =
            std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        // batch check
        let COTSenderOutput { msgs: y_star, .. } = self.rcot.send_correlated(ctx, CSP).await?;

        let checkfr = ctx.io_mut().expect_next().await?;

        let (output, check_msg) =
            Backend::spawn(move || ext_sender.check(&y_star, checkfr)).await?;

        ctx.io_mut().send(check_msg).await?;

        self.state = State::Complete;

        Ok(output)
    }
}
