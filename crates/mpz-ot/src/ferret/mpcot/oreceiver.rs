// use crate::{
//     ferret::{mpcot::error::ReceiverError, spcot::Receiver as SpcotReceiver},
//     RandomCOTReceiver,
// };
// use enum_try_as_inner::EnumTryAsInner;
// use mpz_common::Context;
// use mpz_core::{prg::Prg, Block};
// use mpz_ot_core::ferret::mpcot::receiver::{state, Receiver as ReceiverCore};
// use serio::SinkExt;
// use utils_aio::non_blocking_backend::{Backend, NonBlockingBackend};

// #[derive(Debug, EnumTryAsInner, Default)]
// #[derive_err(Debug)]
// pub(crate) enum State {
//     Initialized(ReceiverCore<state::Initialized>),
//     Extension(ReceiverCore<state::Extension>),
//     Complete,
//     #[default]
//     Error,
// }

// /// MPCOT receiver.
// #[derive(Debug, Default)]
// pub struct Receiver<RandomCOT> {
//     state: State,
//     spcot: SpcotReceiver<RandomCOT>,
// }

// impl<RandomCOT: Send + Default> Receiver<RandomCOT> {
//     /// Creates a new Receiver.
//     ///
//     /// # Arguments
//     ///
//     /// * `rcot` - A rcot receiver.
//     pub fn new() -> Self {
//         Self {
//             state: State::Initialized(ReceiverCore::new()),
//             spcot: crate::ferret::spcot::Receiver::new(),
//         }
//     }

//     /// Performs setup.
//     ///
//     /// # Argument
//     ///
//     /// * `ctx` - The context.
//     pub async fn setup<Ctx: Context>(
//         &mut self,
//         ctx: &mut Ctx,
//         rcot: RandomCOT,
//     ) -> Result<(), ReceiverError> {
//         let ext_receiver =
//             std::mem::replace(&mut self.state, State::Error).try_into_initialized()?;

//         let hash_seed = Prg::new().random_block();

//         let (ext_receiver, hash_seed) = ext_receiver.setup(hash_seed);

//         ctx.io_mut().send(hash_seed).await?;

//         self.state = State::Extension(ext_receiver);
//         self.spcot.setup(rcot)?;

//         Ok(())
//     }

//     /// Performs MPCOT extension.
//     ///
//     ///
//     /// # Arguments
//     ///
//     /// * `ctx` - The context,
//     /// * `alphas` - The queried indices.
//     /// * `n` - The total number of indices.
//     pub async fn extend<Ctx: Context>(
//         &mut self,
//         ctx: &mut Ctx,
//         alphas: &[u32],
//         n: u32,
//     ) -> Result<Vec<Block>, ReceiverError>
//     where
//         RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
//     {
//         let ext_receiver = std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

//         let alphas_vec = alphas.to_vec();
//         let (ext_receiver, h_and_pos) =
//             Backend::spawn(move || ext_receiver.pre_extend(&alphas_vec, n)).await?;

//         let mut hs = vec![0usize; h_and_pos.len()];

//         let mut pos = vec![0u32; h_and_pos.len()];

//         for (index, (h, p)) in h_and_pos.iter().enumerate() {
//             hs[index] = *h;
//             pos[index] = *p;
//         }

//         self.spcot.extend(ctx, &pos, &hs).await?;

//         let rt = self.spcot.check(ctx).await?;

//         let rt: Vec<Vec<Block>> = rt.into_iter().map(|(elem, _)| elem).collect();
//         let (ext_receiver, output) = Backend::spawn(move || ext_receiver.extend(&rt)).await?;

//         self.state = State::Extension(ext_receiver);

//         Ok(output)
//     }

//     /// Compete extension.
//     pub fn finalize(&mut self) -> Result<(), ReceiverError> {
//         std::mem::replace(&mut self.state, State::Error).try_into_extension()?;

//         self.spcot.finalize()?;
//         self.state = State::Complete;

//         Ok(())
//     }
// }
