use crate::{
    kos::{
        msgs::{Check, Extend, SenderPayload},
        ReceiverConfig, ReceiverError, Rng, RngSeed, CSP, SSP,
    },
    matrix::Matrix,
    msgs::Derandomize,
};

use itybity::FromBitIterator;
use mpz_core::{aes::FIXED_KEY_AES, Block};

use blake3::Hasher;
use rand::{thread_rng, Rng as _, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_core::RngCore;
use utils::bits::ToBitsIter;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

#[derive(Debug, Default)]
struct Tape {
    ciphertext_digests: Vec<[u8; 32]>,
}

/// KOS15 receiver.
#[derive(Debug, Default)]
pub struct Receiver<T: state::State = state::Initialized> {
    config: ReceiverConfig,
    state: T,
    /// Protocol tape
    tape: Option<Tape>,
}

impl Receiver {
    /// Creates a new Sender
    ///
    /// # Arguments
    ///
    /// * `config` - The Sender's configuration
    pub fn new(config: ReceiverConfig) -> Self {
        let tape = if config.receiver_commit() {
            Some(Tape::default())
        } else {
            None
        };

        Receiver {
            config,
            state: state::Initialized::default(),
            tape,
        }
    }

    /// Complete the base setup phase of the protocol.
    ///
    /// # Arguments
    ///
    /// * `seeds` - The receiver's rng seeds
    pub fn base_setup(self, seeds: [[Block; 2]; CSP]) -> Receiver<state::Extension> {
        let rngs = seeds
            .iter()
            .map(|seeds| {
                seeds.map(|seed| {
                    let mut seed_ = RngSeed::default();
                    seed_
                        .iter_mut()
                        .zip(seed.to_bytes().into_iter().cycle())
                        .for_each(|(s, c)| *s = c);
                    Rng::from_seed(seed_)
                })
            })
            .collect();

        Receiver {
            config: self.config,
            state: state::Extension {
                rngs,
                keys: Vec::default(),
                choices: Vec::default(),
                counter: 0,
                unchecked_ts: Vec::default(),
                unchecked_keys: Vec::default(),
                unchecked_choices: Vec::default(),
            },
            tape: self.tape,
        }
    }
}

impl Receiver<state::Extension> {
    /// Perform the IKNP OT extension.
    ///
    /// # Sacrificial OTs
    ///
    /// Performing the consistency check sacrifices 256 OTs for the consistency check, so be sure to
    /// extend enough OTs to compensate for this.
    ///
    /// # Streaming
    ///
    /// Extension can be performed in a streaming fashion by calling this method multiple times, sending
    /// the `Extend` messages to the sender in-between calls.
    ///
    /// The freshly extended OTs are not available until after the consistency check has been
    /// performed. See [`Receiver::check`].
    ///
    /// # Arguments
    ///
    /// * `choices` - The receiver's choices
    pub fn extend(&mut self, count: usize) -> Extend {
        // Round up the OTs to extend to the nearest multiple of 64 (matrix transpose optimization).
        let count = (count + 63) & !63;

        let mut rng = thread_rng();
        let choices = (0..count / 8)
            .flat_map(|_| rng.gen::<u8>().into_lsb0_iter())
            .collect::<Vec<_>>();

        let choice_vector = Vec::<u8>::from_lsb0_iter(choices.iter().copied());

        const NROWS: usize = CSP;
        let mut ts = Matrix::new(vec![0u8; NROWS * count / 8], count / 8);
        let mut us = Matrix::new(vec![0u8; NROWS * count / 8], count / 8);

        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")] {
                let iter = self.state.rngs
                    .par_iter_mut()
                    .zip(ts.par_iter_rows_mut())
                    .zip(us.par_iter_rows_mut());
            } else {
                let iter = self.state.rngs
                    .iter_mut()
                    .zip(ts.iter_rows_mut())
                    .zip(us.iter_rows_mut());
            }
        }

        iter.for_each(|((rngs, t), u)| {
            // Figure 3, step 2.
            rngs[0].fill_bytes(t);
            rngs[1].fill_bytes(u);

            // Figure 3, step 3.
            // Computing `u = t_0 + t_1 + x`.
            u.iter_mut()
                .zip(t)
                .zip(&choice_vector)
                .for_each(|((u, t), r)| {
                    *u ^= *t ^ r;
                });
        });

        ts.transpose_bits().expect("matrix is rectangular");

        let cipher = &(*FIXED_KEY_AES);
        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")] {
                let iter = ts.par_iter_rows().enumerate();
            } else {
                let iter = ts.iter_rows().enumerate();
            }
        }

        let (ts, keys): (Vec<_>, Vec<_>) = iter
            .map(|(j, t)| {
                let t: Block = t.try_into().unwrap();
                let j = Block::new(((self.state.counter + j) as u128).to_be_bytes());
                let k = cipher.tccr(j, t);

                (t, k)
            })
            .unzip();

        self.state.counter += count;

        self.state.unchecked_ts.extend(ts);
        self.state.unchecked_keys.extend(keys);
        self.state.unchecked_choices.extend(choices);

        Extend {
            count,
            us: us.into_inner(),
        }
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
    /// coin-toss protocol **after** the receiver has sent their setup message, ie
    /// after they have already committed to their choice vectors.
    ///
    /// # Arguments
    ///
    /// * `chi_seed` - The seed used to generate the consistency check weights.
    pub fn check(&mut self, chi_seed: Block) -> Check {
        let mut seed = RngSeed::default();
        seed.iter_mut()
            .zip(chi_seed.to_bytes().into_iter().cycle())
            .for_each(|(s, c)| *s = c);

        let mut rng = Rng::from_seed(seed);

        let unchecked_ts = std::mem::take(&mut self.state.unchecked_ts);
        let mut unchecked_keys = std::mem::take(&mut self.state.unchecked_keys);
        let mut unchecked_choices = std::mem::take(&mut self.state.unchecked_choices);

        // Figure 7, "Check correlation", point 1.
        // Sample random weights for the consistency check.
        let chis = (0..unchecked_ts.len())
            .map(|_| Block::random(&mut rng))
            .collect::<Vec<_>>();

        // Figure 7, "Check correlation", point 2.
        // Compute the random linear combinations.
        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")] {
                let (x, t0, t1) = unchecked_choices.par_iter()
                    .zip(unchecked_ts)
                    .zip(chis)
                    .map(|((c, t), chi)| {
                        let x = if *c { chi } else { Block::ZERO };
                        let (t0, t1) = t.clmul(chi);
                        (x, t0, t1)
                    })
                    .reduce(
                        || (Block::ZERO, Block::ZERO, Block::ZERO),
                        |(_x, _t0, _t1), (x, t0, t1)| {
                            (_x ^ x, _t0 ^ t0, _t1 ^ t1)
                        },
                    );
            } else {
                use itybity::ToBits;

                let (x, t0, t1) = unchecked_choices.iter()
                    .zip(unchecked_ts)
                    .zip(chis)
                    .map(|((c, t), chi)| {
                        let x = if *c { chi } else { Block::ZERO };
                        let (t0, t1) = t.clmul(chi);
                        (x, t0, t1)
                    })
                    .reduce(|(_x, _t0, _t1), (x, t0, t1)| {
                        (_x ^ x, _t0 ^ t0, _t1 ^ t1)
                    }).unwrap();
            }
        }

        // Strip off the rows sacrificed for the consistency check.
        let nrows = unchecked_keys.len() - (CSP + SSP);
        unchecked_keys.truncate(nrows);
        unchecked_choices.truncate(nrows);

        // Add to existing keys.
        self.state.keys.extend(unchecked_keys);
        self.state.choices.extend(unchecked_choices);

        Check { x, t0, t1 }
    }

    /// Derandomize the receiver's choices from the setup phase.
    ///
    /// # Arguments
    ///
    /// * `choices` - The receiver's corrected choices.
    pub fn derandomize(&self, choices: &[bool]) -> Derandomize {
        let flip = Vec::<u8>::from_lsb0_iter(
            self.state
                .choices
                .iter()
                .zip(choices)
                .map(|(setup_choice, new_choice)| setup_choice ^ new_choice),
        );

        Derandomize { flip }
    }

    /// Obliviously receive the sender's messages.
    ///
    /// # Arguments
    ///
    /// * `payload` - The sender's payload
    pub fn receive(&mut self, payload: SenderPayload) -> Result<Vec<Block>, ReceiverError> {
        let SenderPayload { ciphertexts } = payload;

        if ciphertexts.len() % 2 != 0 {
            return Err(ReceiverError::InvalidPayload);
        }

        let count = ciphertexts.len() / 2;

        if count > self.state.keys.len() {
            return Err(ReceiverError::CountMismatch(self.state.keys.len(), count));
        }

        if let Some(tape) = &mut self.tape {
            let mut hasher = Hasher::default();
            ciphertexts.iter().for_each(|ct| {
                hasher.update(&ct.to_bytes());
            });
            tape.ciphertext_digests.push(hasher.finalize().into());
        }

        let keys = self.state.keys.drain(..count);
        let choices = self.state.choices.drain(..count);

        let plaintexts = keys
            .zip(choices)
            .zip(ciphertexts.chunks(2))
            .map(|((key, c), ct)| if c { key ^ ct[1] } else { key ^ ct[0] })
            .collect();

        Ok(plaintexts)
    }

    /// Checks the purported messages against the receiver's protocol tape, using the sender's
    /// base choices `delta`.
    ///
    /// # ⚠️ Warning ⚠️
    ///
    /// The authenticity of `delta` must be established outside the context of this function. This
    /// can be achieved using verifiable OT for the base choices.
    ///
    /// # Arguments
    ///
    /// * `delta` - The sender's base choices.
    /// * `purported_msgs` - The purported messages sent by the sender.
    pub fn verify(self, delta: Block, purported_msgs: &[[Block; 2]]) -> Result<(), ReceiverError> {
        todo!()
    }
}

/// The receiver's state.
pub mod state {
    use super::*;

    mod sealed {
        pub trait Sealed {}

        impl Sealed for super::Initialized {}
        impl Sealed for super::Extension {}
    }

    /// The receiver's state.
    pub trait State: sealed::Sealed {}

    /// The receiver's initial state.
    #[derive(Default)]
    pub struct Initialized {}

    impl State for Initialized {}

    opaque_debug::implement!(Initialized);

    /// The receiver's state after base setup
    pub struct Extension {
        /// Receiver's rngs
        pub(super) rngs: Vec<[ChaCha20Rng; 2]>,
        /// Receiver's keys
        pub(super) keys: Vec<Block>,
        /// Receiver's random choices
        pub(super) choices: Vec<bool>,
        /// Messages received so far
        pub(super) counter: usize,

        /// Receiver's unchecked ts
        pub(super) unchecked_ts: Vec<Block>,
        /// Receiver's unchecked keys
        pub(super) unchecked_keys: Vec<Block>,
        /// Receiver's unchecked choices
        pub(super) unchecked_choices: Vec<bool>,
    }

    impl State for Extension {}

    opaque_debug::implement!(Extension);
}
