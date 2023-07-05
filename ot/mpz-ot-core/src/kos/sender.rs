use crate::{
    kos::{
        msgs::{Check, Extend, SenderPayload},
        Rng, RngSeed, SenderConfig, SenderError, CSP, SSP,
    },
    matrix::Matrix,
    msgs::Derandomize,
};

use itybity::IntoBitIterator;
use mpz_core::{aes::FIXED_KEY_AES, Block};

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_core::RngCore;

cfg_if::cfg_if! {
    if #[cfg(feature = "rayon")] {
        use itybity::ToParallelBits;
        use rayon::prelude::*;
    } else {
        use itybity::ToBits;
    }
}

/// KOS15 sender.
#[derive(Debug, Default)]
pub struct Sender<T: state::State = state::Initialized> {
    config: SenderConfig,
    state: T,
}

impl Sender {
    /// Creates a new Sender
    ///
    /// # Arguments
    ///
    /// * `config` - The Sender's configuration
    pub fn new(config: SenderConfig) -> Self {
        Sender {
            config,
            state: state::Initialized::default(),
        }
    }

    /// Complete the base setup phase of the protocol.
    ///
    /// # Arguments
    ///
    /// * `delta` - The sender's base OT choices
    /// * `seeds` - The receiver's rng seeds received via base OT
    pub fn base_setup(self, delta: Block, seeds: [Block; CSP]) -> Sender<state::Extension> {
        let rngs = seeds
            .iter()
            .map(|seed| {
                let mut seed_ = RngSeed::default();
                seed_
                    .iter_mut()
                    .zip(seed.to_bytes().into_iter().cycle())
                    .for_each(|(s, c)| *s = c);
                Rng::from_seed(seed_)
            })
            .collect();

        Sender {
            config: self.config,
            state: state::Extension {
                delta,
                rngs,
                qs: Vec::default(),
                keys: Vec::default(),
                counter: 0,
                unchecked_qs: Vec::default(),
                unchecked_keys: Vec::default(),
            },
        }
    }
}

impl Sender<state::Extension> {
    /// Perform the IKNP OT extension.
    ///
    /// # Sacrificial OTs
    ///
    /// Performing the consistency check sacrifices 256 OTs, so be sure to extend enough to
    /// compensate for this.
    ///
    /// # Streaming
    ///
    /// Extension can be performed in a streaming fashion by processing an extension in batches via
    /// multiple calls to this method.
    ///
    /// The freshly extended OTs are not available until after the consistency check has been
    /// performed. See [`Sender::check`].
    ///
    /// # Arguments
    ///
    /// * `count` - The number of additional OTs to extend
    /// * `extend` - The receiver's setup message
    pub fn extend(&mut self, count: usize, extend: Extend) -> Result<(), SenderError> {
        // Round up the OTs to extend to the nearest multiple of 64 (matrix transpose optimization).
        let count = (count + 63) & !63;

        // Make sure the number of OTs to extend matches the receiver's setup message.
        if extend.count != count {
            return Err(SenderError::CountMismatch(extend.count, count));
        }

        const NROWS: usize = CSP;
        let mut unchecked_qs = Matrix::new(vec![0u8; NROWS * count / 8], count / 8);
        let us = Matrix::new(extend.us, count / 8);

        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")] {
                let iter = self.state.delta
                    .par_iter_lsb0()
                    .zip(self.state.rngs.par_iter_mut())
                    .zip(unchecked_qs.par_iter_rows_mut())
                    .zip(us.par_iter_rows());
            } else {
                let iter = self.state.delta
                    .iter_lsb0()
                    .zip(self.state.rngs.iter_mut())
                    .zip(unchecked_qs.iter_rows_mut())
                    .zip(us.iter_rows());
            }
        }

        let zero = vec![0u8; count / 8];
        iter.for_each(|(((b, rng), q), u)| {
            rng.fill_bytes(q);
            // If `b` is true, xor `u` into `q`, otherwise xor 0 into `q` (constant time).
            let u = if b { u } else { &zero };
            q.iter_mut().zip(u).for_each(|(q, u)| *q ^= u);
        });

        unchecked_qs
            .transpose_bits()
            .expect("matrix is rectangular");

        self.state.unchecked_qs.extend(unchecked_qs);

        Ok(())
    }

    /// Checks the consistency of the receiver's choice vectors for all outstanding OTs.
    ///
    /// See section 3.1 of the paper for more details.
    ///
    /// # Sacrificial OTs
    ///
    /// Performing this check sacrifices 256 OTs for the consistency check, so be sure to
    /// extend enough OTs to compensate for this.
    ///
    /// # ⚠️ Warning ⚠️
    ///
    /// The provided seed must be unbiased! It should be generated using a secure
    /// coin-toss protocol **after** the receiver has sent their extension message, ie
    /// after they have already committed to their choice vectors.
    ///
    /// # Arguments
    ///
    /// * `chi_seed` - The seed used to generate the consistency check weights.
    /// * `receiver_check` - The receiver's consistency check message.
    pub fn check(&mut self, chi_seed: Block, receiver_check: Check) -> Result<(), SenderError> {
        let mut seed = RngSeed::default();
        seed.iter_mut()
            .zip(chi_seed.to_bytes().into_iter().cycle())
            .for_each(|(s, c)| *s = c);

        let mut rng = Rng::from_seed(seed);

        let mut unchecked_qs = self.state.unchecked_qs.take();

        // Figure 7, "Check correlation", point 1.
        // Sample random weights for the consistency check.
        let chis = (0..unchecked_qs.rows())
            .map(|_| Block::random(&mut rng))
            .collect::<Vec<_>>();

        // Figure 7, "Check correlation", point 3.
        // Compute the random linear combinations.
        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")] {
                let check = unchecked_qs.par_iter_rows()
                    .zip(chis)
                    .map(|(q, chi)| {
                        let q: Block = q.try_into().unwrap();
                        q.clmul(chi)
                    })
                    .reduce(
                        || (Block::ZERO, Block::ZERO),
                        |(_a, _b), (a, b)| (a ^ _a, b ^ _b),
                    );
            } else {
                let check = unchecked_qs.iter_rows()
                    .zip(chis)
                    .map(|(q, chi)| {
                        let q: Block = q.try_into().unwrap();
                        q.clmul(chi)
                    })
                    .reduce(
                        |(_a, _b), (a, b)| (a ^ _a, b ^ _b),
                    ).unwrap();
            }
        }

        let Check { x, t0, t1 } = receiver_check;
        let tmp = x.clmul(self.state.delta);
        let check = (check.0 ^ tmp.0, check.1 ^ tmp.1);

        // Call the police!
        if check != (t0, t1) {
            return Err(SenderError::ConsistencyCheckFailed);
        }

        // Strip off the rows sacrificed for the consistency check.
        let nrows = unchecked_qs.rows() - (CSP + SSP);
        unchecked_qs.truncate_rows(nrows);

        // Add to existing qs.
        self.state.qs.extend(unchecked_qs);

        Ok(())
    }

    /// Obliviously transfers the provided messages to the receiver, applying Beaver
    /// derandomization to correct the receiver's choices made during extension.
    ///
    /// # Arguments
    ///
    /// * `msgs` - The messages to obliviously transfer
    /// * `derandomize` - The receiver's derandomization choices
    pub fn send(
        &mut self,
        msgs: &[[Block; 2]],
        derandomize: Derandomize,
    ) -> Result<SenderPayload, SenderError> {
        let Derandomize { flip } = derandomize;

        let flip = flip.into_lsb0_vec();

        if msgs.len() > flip.len() {
            return Err(SenderError::CountMismatch(flip.len(), msgs.len()));
        }

        if msgs.len() > self.state.qs.rows() {
            return Err(SenderError::InsufficientSetup(
                msgs.len(),
                self.state.qs.rows(),
            ));
        }

        let qs = self.state.qs.drain_rows(0..msgs.len());
        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")] {
                let iter = qs.par_iter_rows()
                    .enumerate();
            } else {
                let iter = qs.iter_rows()
                    .enumerate();
            }
        }

        // Encrypt the chosen messages using the generated keys from ROT.
        let cipher = &(*FIXED_KEY_AES);
        let ciphertexts = iter
            .zip(msgs)
            .zip(flip)
            .flat_map(|(((j, q), [m0, m1]), flip)| {
                let q: Block = q.try_into().unwrap();
                let j = Block::new(((self.state.counter + j) as u128).to_be_bytes());

                // Figure 7, "Randomize".
                let k0 = cipher.tccr(j, q);
                let k1 = cipher.tccr(j, q ^ self.state.delta);

                // Use Beaver derandomization to correct the receiver's choices
                // from the extension phase.
                if flip {
                    [k0 ^ *m1, k1 ^ *m0]
                } else {
                    [k0 ^ *m0, k1 ^ *m1]
                }
            })
            .collect();

        self.state.counter += msgs.len();

        Ok(SenderPayload { ciphertexts })
    }
}

/// The sender's state.
pub mod state {
    use super::*;

    mod sealed {
        pub trait Sealed {}

        impl Sealed for super::Initialized {}
        impl Sealed for super::Extension {}
    }

    /// The sender's state.
    pub trait State: sealed::Sealed {}

    /// The sender's initial state.
    #[derive(Default)]
    pub struct Initialized {}

    impl State for Initialized {}

    opaque_debug::implement!(Initialized);

    /// The sender's state after base setup
    pub struct Extension {
        /// Sender's base OT choices
        pub(super) delta: Block,
        /// Receiver's rngs seeded from base OT
        pub(super) rngs: Vec<ChaCha20Rng>,
        /// Sender's share of the extended OTs
        pub(super) qs: Vec<Block>,
        /// Sender's keys
        pub(super) keys: Vec<Block>,
        /// Number of OT's sent so far
        pub(super) counter: usize,

        /// Sender's unchecked qs
        pub(super) unchecked_qs: Vec<Block>,
        /// Sender's unchecked keys
        pub(super) unchecked_keys: Vec<Block>,
    }

    impl State for Extension {}

    opaque_debug::implement!(Extension);
}
