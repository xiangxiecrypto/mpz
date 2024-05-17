//! SPCOT receiver
use crate::ferret::{spcot::error::ReceiverError, CSP};
use itybity::ToBits;
use mpz_core::{
    aes::FIXED_KEY_AES, ggm_tree::GgmTree, hash::Hash, prg::Prg, serialize::CanonicalSerialize,
    utils::blake3, Block,
};
use rand_core::SeedableRng;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};

use super::msgs::{CheckFromReceiver, CheckFromSender, ExtendFromSender, MaskBits};

/// SPCOT receiver.
#[derive(Debug, Default)]
pub struct Receiver<T: state::State = state::Initialized> {
    state: T,
}

impl Receiver {
    /// Creates a new Receiver.
    pub fn new() -> Self {
        Receiver {
            state: state::Initialized::default(),
        }
    }

    /// Completes the setup phase of the protocol.
    ///
    /// See step 1 in Figure 6.
    ///
    pub fn setup(self) -> Receiver<state::Extension> {
        Receiver {
            state: state::Extension {
                unchecked_ws: Vec::default(),
                chis: Vec::default(),
                alphas_and_length: Vec::default(),
                cot_counter: 0,
                exec_counter: 0,
                extended: false,
                hasher: blake3::Hasher::new(),
            },
        }
    }
}

impl Receiver<state::Extension> {
    /// Performs the mask bit step in batch in extension.
    ///
    /// See step 4 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `hs` - The depths of the GGM trees.
    /// * `alphas` - The vector of chosen positions.
    /// * `rss` - The message from COT ideal functionality for the receiver for all the tress. Only the random bits are used.
    pub fn extend_mask_bits(
        &mut self,
        hs: &[usize],
        alphas: &[u32],
        rss: &[bool],
    ) -> Result<Vec<MaskBits>, ReceiverError> {
        if self.state.extended {
            return Err(ReceiverError::InvalidState(
                "extension is not allowed".to_string(),
            ));
        }

        if alphas.len() != hs.len() {
            return Err(ReceiverError::InvalidLength(
                "the length of alphas should be the length of hs".to_string(),
            ));
        }

        if alphas
            .iter()
            .zip(hs.iter())
            .any(|(alpha, h)| *alpha >= (1 << h))
        {
            return Err(ReceiverError::InvalidInput(
                "the input pos should be no more than 2^h-1".to_string(),
            ));
        }

        let h_sum = hs.iter().sum();

        if rss.len() != h_sum {
            return Err(ReceiverError::InvalidLength(
                "the length of r should be the sum of h".to_string(),
            ));
        }

        let mut rs_s = vec![Vec::<bool>::new(); hs.len()];
        let mut rss_vec = rss.to_vec();
        for (index, h) in hs.iter().enumerate() {
            rs_s[index] = rss_vec.drain(0..*h).collect();
        }

        // Step 4 in Figure 6
        let mut bss = vec![Vec::<bool>::new(); hs.len()];

        let iter = bss
            .iter_mut()
            .zip(alphas.iter())
            .zip(hs.iter())
            .zip(rs_s.iter())
            .map(|(((bs, alpha), h), rs)| (bs, alpha, h, rs));

        for (bs, alpha, h, rs) in iter {
            *bs = alpha
                .iter_msb0()
                .skip(32 - h)
                // Computes alpha_i XOR r_i XOR 1.
                .zip(rs.iter())
                .map(|(alpha, &r)| alpha == r)
                .collect();
        }

        // Updates hasher.
        self.state.hasher.update(&bss.to_bytes());

        let res: Vec<MaskBits> = bss.into_iter().map(|bs| MaskBits { bs }).collect();

        Ok(res)
    }

    /// Performs the GGM reconstruction step in batch in extension. This function can be called multiple times before checking.
    ///
    /// See step 5 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `hs` - The depths of the GGM trees.
    /// * `alphas` - The vector of chosen positions.
    /// * `tss` - The message from COT ideal functionality for the receiver. Only the chosen blocks are used.
    /// * `extendfss` - The vector of messages sent by the sender.
    pub fn extend(
        &mut self,
        hs: &[usize],
        alphas: &[u32],
        tss: &[Block],
        extendfss: &[ExtendFromSender],
    ) -> Result<(), ReceiverError> {
        if self.state.extended {
            return Err(ReceiverError::InvalidState(
                "extension is not allowed".to_string(),
            ));
        }

        if alphas.len() != hs.len() {
            return Err(ReceiverError::InvalidLength(
                "the length of alphas should be the length of hs".to_string(),
            ));
        }

        if alphas
            .iter()
            .zip(hs.iter())
            .any(|(alpha, h)| *alpha >= (1 << h))
        {
            return Err(ReceiverError::InvalidInput(
                "the input pos should be no more than 2^h-1".to_string(),
            ));
        }

        let h_sum = hs.iter().sum();

        if tss.len() != h_sum {
            return Err(ReceiverError::InvalidLength(
                "the length of tss should be the sum of h".to_string(),
            ));
        }

        let mut ts_s = vec![Vec::<Block>::new(); hs.len()];
        let mut tss_vec = tss.to_vec();
        for (index, h) in hs.iter().enumerate() {
            ts_s[index] = tss_vec.drain(0..*h).collect();
        }

        if extendfss.len() != hs.len() {
            return Err(ReceiverError::InvalidLength(
                "the length of extendfss should be the length of hs".to_string(),
            ));
        }

        let mut ms_s = vec![Vec::<[Block; 2]>::new(); hs.len()];
        let mut sum_s = vec![Block::ZERO; hs.len()];

        for (index, extendfs) in extendfss.iter().enumerate() {
            ms_s[index] = extendfs.ms.clone();
            sum_s[index] = extendfs.sum;
        }

        if ms_s.iter().zip(hs.iter()).any(|(ms, h)| ms.len() != *h) {
            return Err(ReceiverError::InvalidLength(
                "the length of ms should be h".to_string(),
            ));
        }
        // Updates hasher
        self.state.hasher.update(&ms_s.to_bytes());
        self.state.hasher.update(&sum_s.to_bytes());

        let mut trees = vec![Vec::<Block>::new(); hs.len()];

        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")]{
                let iter = alphas
                .par_iter()
                .zip(ms_s.par_iter())
                .zip(sum_s.par_iter())
                .zip(hs.par_iter())
                .zip(ts_s.par_iter())
                .zip(trees.par_iter_mut())
                .map(|(((((alpha, ms), sum), h), ts), tree)| (alpha, ms, sum, h, ts, tree));
            }else{
                let iter = alphas
                .iter()
                .zip(ms_s.iter())
                .zip(sum_s.iter())
                .zip(hs.iter())
                .zip(ts_s.iter())
                .zip(trees.iter_mut())
                .map(|(((((alpha, ms), sum), h), ts), tree)| (alpha, ms, sum, h, ts, tree));
            }
        }

        iter.for_each(|(alpha, ms, sum, h, ts, tree)| {
            let alpha_bar_vec: Vec<bool> = alpha.iter_msb0().skip(32 - h).map(|a| !a).collect();

            // Step 5 in Figure 6.
            let k: Vec<Block> = ms
                .iter()
                .zip(ts)
                .zip(alpha_bar_vec.iter())
                .enumerate()
                .map(|(i, (([m0, m1], &t), &b))| {
                    let tweak: Block = bytemuck::cast([i, self.state.exec_counter]);
                    if !b {
                        // H(t, i|ell) ^ M0
                        FIXED_KEY_AES.tccr(tweak, t) ^ *m0
                    } else {
                        // H(t, i|ell) ^ M1
                        FIXED_KEY_AES.tccr(tweak, t) ^ *m1
                    }
                })
                .collect();

            // Reconstructs GGM tree except `ws[alpha]`.
            let ggm_tree = GgmTree::new(*h);
            *tree = vec![Block::ZERO; 1 << h];
            ggm_tree.reconstruct(tree, &k, &alpha_bar_vec);

            // Sets `tree[alpha]`, which is `ws[alpha]`.
            tree[(*alpha) as usize] = tree.iter().fold(*sum, |acc, &x| acc ^ x);
        });

        for tree in trees {
            self.state.unchecked_ws.extend_from_slice(&tree);
        }

        for (alpha, h) in alphas.iter().zip(hs.iter()) {
            self.state.alphas_and_length.push((*alpha, 1 << h));
        }

        self.state.exec_counter += hs.len();

        Ok(())
    }

    /// Performs the decomposition and bit-mask steps in check.
    ///
    /// See step 7 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `x_star` - The message from COT ideal functionality for the receiver. Only the random bits are used.
    pub fn check_pre(&mut self, x_star: &[bool]) -> Result<CheckFromReceiver, ReceiverError> {
        if x_star.len() != CSP {
            return Err(ReceiverError::InvalidLength(format!(
                "the length of x* should be {CSP}"
            )));
        }

        let seed = *self.state.hasher.finalize().as_bytes();
        let mut prg = Prg::from_seed(Block::try_from(&seed[0..16]).unwrap());

        // The sum of all the chi[alpha].
        let mut sum_chi_alpha = Block::ZERO;

        for (alpha, n) in &self.state.alphas_and_length {
            let mut chis = vec![Block::ZERO; *n as usize];
            prg.random_blocks(&mut chis);
            sum_chi_alpha ^= chis[*alpha as usize];
            self.state.chis.extend_from_slice(&chis);
        }

        let x_prime: Vec<bool> = sum_chi_alpha
            .iter_lsb0()
            .zip(x_star)
            .map(|(x, &x_star)| x != x_star)
            .collect();

        Ok(CheckFromReceiver { x_prime })
    }

    /// Performs the final step of the consistency check.
    ///
    /// See step 9 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `z_star` - The message from COT ideal functionality for the receiver. Only the chosen blocks are used.
    /// * `check` - The hashed value sent by the Sender.
    pub fn check(
        &mut self,
        z_star: &[Block],
        check: CheckFromSender,
    ) -> Result<Vec<(Vec<Block>, u32)>, ReceiverError> {
        let CheckFromSender { hashed_v } = check;

        if z_star.len() != CSP {
            return Err(ReceiverError::InvalidLength(format!(
                "the length of z* should be {CSP}"
            )));
        }

        // Computes the base X^i
        let base: Vec<Block> = (0..CSP).map(|x| bytemuck::cast((1_u128) << x)).collect();

        // Computes Z.
        let mut w = Block::inn_prdt_red(z_star, &base);

        // Computes W.
        w ^= Block::inn_prdt_red(&self.state.chis, &self.state.unchecked_ws);

        // Computes H'(W)
        let hashed_w = Hash::from(blake3(&w.to_bytes()));

        if hashed_v != hashed_w {
            return Err(ReceiverError::ConsistencyCheckFailed);
        }

        self.state.cot_counter += self.state.unchecked_ws.len();

        let mut res = Vec::new();
        for (alpha, n) in &self.state.alphas_and_length {
            let tmp: Vec<Block> = self.state.unchecked_ws.drain(..*n as usize).collect();
            res.push((tmp, *alpha));
        }

        self.state.hasher = blake3::Hasher::new();
        self.state.alphas_and_length.clear();
        self.state.chis.clear();
        self.state.unchecked_ws.clear();
        
        Ok(res)
    }

    /// Complete extension.
    #[inline]
    pub fn finalize(&mut self) {
        self.state.extended = true;
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

    /// The receiver's state after the setup phase.
    ///
    /// In this state the receiver performs COT extension and outputs random choice bits (potentially multiple times).
    pub struct Extension {
        /// Receiver's output blocks.
        pub(super) unchecked_ws: Vec<Block>,
        /// Receiver's random challenges chis.
        pub(super) chis: Vec<Block>,
        /// Stores the alpha and the length in each extend phase.
        pub(super) alphas_and_length: Vec<(u32, u32)>,

        /// Current COT counter
        pub(super) cot_counter: usize,
        /// Current execution counter
        pub(super) exec_counter: usize,
        /// This is to prevent the receiver from extending twice
        pub(super) extended: bool,

        /// A hasher to generate chi seed from the protocol transcript.
        pub(super) hasher: blake3::Hasher,
    }

    impl State for Extension {}

    opaque_debug::implement!(Extension);
}
