//! SPCOT sender.
use crate::ferret::{spcot::error::SenderError, CSP};
use mpz_core::{
    aes::FIXED_KEY_AES, ggm_tree::GgmTree, hash::Hash, prg::Prg, serialize::CanonicalSerialize,
    utils::blake3, Block,
};
use rand_core::SeedableRng;
#[cfg(feature = "rayon")]
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};

use super::msgs::{CheckFromReceiver, CheckFromSender, ExtendFromSender, MaskBits};

/// SPCOT sender.
#[derive(Debug, Default)]
pub struct Sender<T: state::State = state::Initialized> {
    state: T,
}

impl Sender {
    /// Creates a new Sender.
    pub fn new() -> Self {
        Sender {
            state: state::Initialized::default(),
        }
    }

    /// Completes the setup phase of the protocol.
    ///
    /// See step 1 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `delta` - The sender's global secret.
    pub fn setup(self, delta: Block) -> Sender<state::Extension> {
        Sender {
            state: state::Extension {
                delta,
                unchecked_vs: Vec::default(),
                vs_length: Vec::default(),
                cot_counter: 0,
                exec_counter: 0,
                extended: false,
                hasher: blake3::Hasher::new(),
            },
        }
    }
}

impl Sender<state::Extension> {
    /// Performs batch SPCOT extension.
    ///
    /// See Step 1-5 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `hs` - The depths of the GGM trees.
    /// * `qss`- The blocks received by calling the COT functionality for hs trees.
    /// * `masks`- The vector of mask bits sent by the receiver.
    pub fn extend(
        &mut self,
        hs: &[usize],
        qss: &[Block],
        masks: &[MaskBits],
    ) -> Result<Vec<ExtendFromSender>, SenderError> {
        if self.state.extended {
            return Err(SenderError::InvalidState(
                "extension is not allowed".to_string(),
            ));
        }

        let h_sum = hs.iter().sum();

        if qss.len() != h_sum {
            return Err(SenderError::InvalidLength(
                "the length of qss should be the sum of h".to_string(),
            ));
        }

        let mut qs_s = vec![Vec::<Block>::new(); hs.len()];
        let mut qss_vec = qss.to_vec();
        for (index, h) in hs.iter().enumerate() {
            qs_s[index] = qss_vec.drain(0..*h).collect();
        }

        if masks.len() != hs.len() {
            return Err(SenderError::InvalidLength(
                "the length of masks should be the length of hs".to_string(),
            ));
        }

        let bss: Vec<Vec<bool>> = masks.iter().map(|m| m.clone().bs).collect();

        if bss.iter().zip(hs.iter()).any(|(b, h)| b.len() != *h) {
            return Err(SenderError::InvalidLength(
                "the length of b should be h".to_string(),
            ));
        }

        // Updates hasher.
        self.state.hasher.update(&bss.to_bytes());

        // Step 3-4, Figure 6.

        // Generates a GGM tree with depth h and seed s.
        let mut trees = vec![Vec::<Block>::new(); hs.len()];
        let mut ms_s = vec![Vec::<[Block; 2]>::new(); hs.len()];
        let mut sum_s = vec![Block::ZERO; hs.len()];

        cfg_if::cfg_if! {
            if #[cfg(feature = "rayon")]{
                let iter = trees
                .par_iter_mut().zip(hs.par_iter())
                .zip(qs_s.par_iter())
                .zip(bss.par_iter())
                .zip(ms_s.par_iter_mut())
                .zip(sum_s.par_iter_mut())
                .map(|(((((tree, h), qs), bs), ms), sum)| (tree, h, qs, bs, ms, sum));
            }else{
                let iter = trees
                .iter_mut()
                .zip(hs.iter())
                .zip(qs_s.iter())
                .zip(bss.iter())
                .zip(ms_s.iter_mut())
                .zip(sum_s.iter_mut())
                .map(|(((((tree, h), qs), bs), ms), sum)| (tree, h, qs, bs, ms, sum));
            }
        }

        iter.for_each(|(tree, h, qs, bs, ms, sum)| {
            let s = Prg::new().random_block();
            let ggm_tree = GgmTree::new(*h);
            let mut k0 = vec![Block::ZERO; *h];
            let mut k1 = vec![Block::ZERO; *h];
            *tree = vec![Block::ZERO; 1 << h];
            ggm_tree.gen(s, tree, &mut k0, &mut k1);

            // Computes the sum of the leaves and delta.
            *sum = tree.iter().fold(self.state.delta, |acc, &x| acc ^ x);

            // Computes M0 and M1.
            for (((i, &q), b), (k0, k1)) in
                qs.iter().enumerate().zip(bs).zip(k0.into_iter().zip(k1))
            {
                let mut m = if *b {
                    [q ^ self.state.delta, q]
                } else {
                    [q, q ^ self.state.delta]
                };
                let tweak: Block = bytemuck::cast([i, self.state.exec_counter]);
                FIXED_KEY_AES.tccr_many(&[tweak, tweak], &mut m);
                m[0] ^= k0;
                m[1] ^= k1;
                ms.push(m);
            }
        });

        // Stores the tree, i.e., the possible output of sender.
        for tree in trees {
            self.state.unchecked_vs.extend_from_slice(&tree);
        }

        // Stores the length of this extension.
        for h in hs {
            self.state.vs_length.push(1 << h);
        }

        // Updates hasher
        self.state.hasher.update(&ms_s.to_bytes());
        self.state.hasher.update(&sum_s.to_bytes());

        self.state.exec_counter += hs.len();

        let res: Vec<ExtendFromSender> = ms_s
            .into_iter()
            .zip(sum_s.iter())
            .map(|(ms, &sum)| ExtendFromSender { ms, sum })
            .collect();

        Ok(res)
    }

    /// Performs the consistency check for the resulting COTs.
    ///
    /// See Step 6-9 in Figure 6.
    ///
    /// # Arguments
    ///
    /// * `y_star` - The blocks received from the ideal functionality for the check.
    /// * `checkfr` - The bits received from the receiver for the check.
    pub fn check(
        &mut self,
        y_star: &[Block],
        checkfr: CheckFromReceiver,
    ) -> Result<(Vec<Vec<Block>>, CheckFromSender), SenderError> {
        let CheckFromReceiver { x_prime } = checkfr;

        if y_star.len() != CSP {
            return Err(SenderError::InvalidLength(format!(
                "the length of y* should be {CSP}"
            )));
        }

        if x_prime.len() != CSP {
            return Err(SenderError::InvalidLength(format!(
                "the length of x' should be {CSP}"
            )));
        }

        // Step 8 in Figure 6.

        // Computes y = y_star + x' * Delta
        let y: Vec<Block> = y_star
            .iter()
            .zip(x_prime.iter())
            .map(|(&y, &x)| if x { y ^ self.state.delta } else { y })
            .collect();

        // Computes the base X^i
        let base: Vec<Block> = (0..CSP).map(|x| bytemuck::cast((1_u128) << x)).collect();

        // Computes Y
        let mut v = Block::inn_prdt_red(&y, &base);

        // Computes V
        let seed = *self.state.hasher.finalize().as_bytes();
        let mut prg = Prg::from_seed(Block::try_from(&seed[0..16]).unwrap());

        let mut chis = Vec::new();
        for n in &self.state.vs_length {
            let mut chi = vec![Block::ZERO; *n as usize];
            prg.random_blocks(&mut chi);
            chis.extend_from_slice(&chi);
        }
        v ^= Block::inn_prdt_red(&chis, &self.state.unchecked_vs);

        // Computes H'(V)
        let hashed_v = Hash::from(blake3(&v.to_bytes()));

        self.state.cot_counter += self.state.unchecked_vs.len();

        let mut res = Vec::new();
        for n in &self.state.vs_length {
            let tmp: Vec<Block> = self.state.unchecked_vs.drain(..*n as usize).collect();
            res.push(tmp);
        }

        self.state.hasher = blake3::Hasher::new();
        self.state.unchecked_vs.clear();
        self.state.vs_length.clear();

        Ok((res, CheckFromSender { hashed_v }))
    }

    /// Complete extension.
    #[inline]
    pub fn finalize(&mut self) {
        self.state.extended = true;
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

    /// The sender's state after the setup phase.
    ///
    /// In this state the sender performs COT extension with random choice bits (potentially multiple times). Also in this state the sender responds to COT requests.
    pub struct Extension {
        /// Sender's global secret.
        pub(super) delta: Block,
        /// Sender's output blocks, support multiple extensions.
        pub(super) unchecked_vs: Vec<Block>,
        /// Store the length of each extension.
        pub(super) vs_length: Vec<u32>,

        /// Current COT counter
        pub(super) cot_counter: usize,
        /// Current execution counter
        pub(super) exec_counter: usize,
        /// This is to prevent the receiver from extending twice
        pub(super) extended: bool,

        /// A hasher to generate chi seed.
        pub(super) hasher: blake3::Hasher,
    }

    impl State for Extension {}

    opaque_debug::implement!(Extension);
}
