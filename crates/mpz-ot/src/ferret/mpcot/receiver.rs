use crate::{
    ferret::{mpcot::error::ReceiverError, spcot::Receiver as SpcotReceiver},
    RandomCOTReceiver,
};
use enum_try_as_inner::EnumTryAsInner;

use mpz_common::Context;
use mpz_core::{prg::Prg, Block};
use mpz_ot_core::ferret::{
    mpcot::{
        receiver::{state as uniform_state, Receiver as UniformReceiverCore},
        receiver_regular::{state as regular_state, Receiver as RegularReceiverCore},
    },
    LpnType,
};
use serio::SinkExt;
use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

#[derive(Debug, EnumTryAsInner)]
#[derive_err(Debug)]
pub(crate) enum State {
    UniformInitialized(UniformReceiverCore<uniform_state::Initialized>),
    UniformExtension(UniformReceiverCore<uniform_state::Extension>),
    RegularInitialized(RegularReceiverCore<regular_state::Initialized>),
    RegularExtension(RegularReceiverCore<regular_state::Extension>),
    Complete,
    Error,
}

/// MPCOT receiver.
#[derive(Debug)]
pub(crate) struct Receiver<RandomCOT> {
    state: State,
    spcot: SpcotReceiver<RandomCOT>,
    lpn_type: LpnType,
}

impl<RandomCOT: Send + Default> Receiver<RandomCOT> {
    /// Creates a new Sender.
    ///
    /// # Arguments.
    ///
    /// * `lpn_type` - The type of LPN.
    pub(crate) fn new(lpn_type: LpnType) -> Self {
        match lpn_type {
            LpnType::Uniform => Self {
                state: State::UniformInitialized(UniformReceiverCore::new()),
                spcot: crate::ferret::spcot::Receiver::new(),
                lpn_type,
            },
            LpnType::Regular => Self {
                state: State::RegularInitialized(RegularReceiverCore::new()),
                spcot: crate::ferret::spcot::Receiver::new(),
                lpn_type,
            },
        }
    }

    /// Performs setup for receiver.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context.
    /// * `rcot` - The random COT used by Receiver.
    pub(crate) async fn setup<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        rcot: RandomCOT,
    ) -> Result<(), ReceiverError> {
        match self.lpn_type {
            LpnType::Uniform => {
                let ext_receiver = std::mem::replace(&mut self.state, State::Error)
                    .try_into_uniform_initialized()?;

                let hash_seed = Prg::new().random_block();

                let (ext_receiver, hash_seed) = ext_receiver.setup(hash_seed);

                ctx.io_mut().send(hash_seed).await?;

                self.state = State::UniformExtension(ext_receiver);
            }
            LpnType::Regular => {
                let ext_receiver = std::mem::replace(&mut self.state, State::Error)
                    .try_into_regular_initialized()?;

                let ext_receiver = ext_receiver.setup();

                self.state = State::RegularExtension(ext_receiver);
            }
        }

        self.spcot.setup(rcot)?;

        Ok(())
    }

    /// Performs MPCOT extension.
    ///
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context,
    /// * `alphas` - The queried indices.
    /// * `n` - The total number of indices.
    pub(crate) async fn extend<Ctx: Context>(
        &mut self,
        ctx: &mut Ctx,
        alphas: &[u32],
        n: u32,
    ) -> Result<Vec<Block>, ReceiverError>
    where
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let alphas_vec = alphas.to_vec();

        match self.lpn_type {
            LpnType::Uniform => {
                let ext_receiver = std::mem::replace(&mut self.state, State::Error)
                    .try_into_uniform_extension()?;

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
                let (ext_receiver, output) =
                    Backend::spawn(move || ext_receiver.extend(&rt)).await?;

                self.state = State::UniformExtension(ext_receiver);

                Ok(output)
            }

            LpnType::Regular => {
                let ext_receiver = std::mem::replace(&mut self.state, State::Error)
                    .try_into_regular_extension()?;

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
                let (ext_receiver, output) =
                    Backend::spawn(move || ext_receiver.extend(&rt)).await?;

                self.state = State::RegularExtension(ext_receiver);

                Ok(output)
            }
        }
    }

    /// Complete extension.
    pub(crate) fn finalize(&mut self) -> Result<(), ReceiverError> {
        match self.lpn_type {
            LpnType::Uniform => {
                std::mem::replace(&mut self.state, State::Error).try_into_uniform_extension()?;
            }
            LpnType::Regular => {
                std::mem::replace(&mut self.state, State::Error).try_into_regular_extension()?;
            }
        }

        self.spcot.finalize()?;
        self.state = State::Complete;

        Ok(())
    }
}
