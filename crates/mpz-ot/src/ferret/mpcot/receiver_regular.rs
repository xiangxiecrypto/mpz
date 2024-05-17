use crate::{ferret::spcot::Receiver as SpcotReceiver, RandomCOTReceiver};
use enum_try_as_inner::EnumTryAsInner;

use mpz_common::Context;
use mpz_core::Block;
use mpz_ot_core::ferret::mpcot::receiver_regular::{state, Receiver as ReceiverCore};
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

use super::error::ReceiverRegularError;

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    Initialized(ReceiverCore<state::Initialized>),
    Extension(ReceiverCore<state::Extension>),
    Complete,
    Error,
}

/// MPCOT regular receiver.
#[derive(Debug)]
pub struct ReceiverRegular<RandomCOT> {
    state: State,
    spcot: SpcotReceiver<RandomCOT>,
}

impl<RandomCOT: Send> ReceiverRegular<RandomCOT> {
    /// Creates a new Receiver.
    ///
    /// # Arguments.
    ///
    /// * `rcot` - A rcot receiver.
    pub fn new(rcot: RandomCOT) -> Self {
        Self {
            state: State::Initialized(ReceiverCore::new()),
            spcot: crate::ferret::spcot::Receiver::new(rcot),
        }
    }

    /// Performs setup.
    pub fn setup(&mut self) -> Result<(), ReceiverRegularError> {
        let ext_receiver =
            std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

        let ext_receiver = ext_receiver.setup();

        self.state = State::Extension(ext_receiver);
        self.spcot.setup()?;

        Ok(())
    }

    /// Performs MPCOT regular extension.
    ///
    /// # Argument
    ///
    /// * `ctx` - The context.
    /// * `alphas` - The queried indices.
    /// * `n` - The total number of indices.
    pub async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        alphas: &[u32],
        n: u32,
    ) -> Result<Vec<Block>, ReceiverRegularError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let ext_receiver = std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        let alphas_vec = alphas.to_vec();
        let (ext_receiver, h_and_pos) =
            Backend::spawn(move || ext_receiver.pre_extend(&alphas_vec, n)).await?;

        let mut hs = vec![0usize; h_and_pos.len()];

        let mut pos = vec![0u32; h_and_pos.len()];
        for (index, (h, p)) in h_and_pos.iter().enumerate() {
            hs[index] = *h;
            pos[index] = *p;
        }

        self.spcot.extend(ctx, &pos, &hs).await?;

        let rt = self.spcot.check(ctx).await?;

        let rt: Vec<Vec<Block>> = rt.into_iter().map(|(elem, _)| elem).collect();
        let (ext_receiver, output) = Backend::spawn(move || ext_receiver.extend(&rt)).await?;

        self.state = State::Extension(ext_receiver);

        Ok(output)
    }

    /// Compete extension.
    pub fn finalize(&mut self) -> Result<(), ReceiverRegularError> {
        std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

        self.spcot.finalize()?;
        self.state = State::Complete;

        Ok(())
    }
}
