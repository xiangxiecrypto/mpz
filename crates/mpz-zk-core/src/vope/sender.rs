//! VOPE sender.
use mpz_core::Block;

use crate::vope::CSP;

use super::error::SenderError;

/// VOPE sender
/// This is the verifier in Figure 4.
#[derive(Debug, Default)]
pub struct Sender<T: state::State = state::Initialized> {
    state: T,
}

impl Sender {
    /// Creates a new sender.
    pub fn new() -> Self {
        Sender {
            state: state::Initialized::default(),
        }
    }

    /// Completes the setup phase of the protocol.
    ///
    /// See Initialize in Figure 4.
    ///
    /// # Arguments.
    ///
    /// * `delta` - The sender's global secret.
    pub fn setup(self, delta: Block) -> Sender<state::Extension> {
        Sender {
            state: state::Extension {
                delta,
                vope_counter: 0,
                exec_counter: 0,
            },
        }
    }
}

impl Sender<state::Extension> {
    /// Performs VOPE extension.
    ///
    /// See step 1-3 in Figure 4.
    ///
    /// # Arguments
    ///
    /// * `ks` - The blocks received by calling the COT ideal functionality.
    /// * `d` - The degree of the polynomial.
    ///
    /// Note that this functionality is only suitable for small d.
    pub fn extend(&mut self, ks: &[Block], d: usize) -> Result<Block, SenderError> {
        if d == 0 {
            return Err(SenderError::InvalidInput(
                "the degree d should not be 0".to_string(),
            ));
        }

        if ks.len() != (2 * d - 1) * CSP {
            return Err(SenderError::InvalidLength(
                "the length of ks should be (2 * d -1) * CSP".to_string(),
            ));
        }

        let mut ki = vec![Block::ZERO; 2 * d - 1];

        let base: Vec<Block> = (0..CSP)
            .map(|x| bytemuck::cast((1_u128) << (CSP - 1 - x)))
            .collect();

        let mut h_ks = ks.to_vec();

        for k in ki.iter_mut().take(2 * d - 1) {
            let buf = h_ks.split_off(CSP);
            *k = Block::inn_prdt_red(&h_ks, &base);
            h_ks = buf;
        }

        let mut b = ki[0];

        for i in 0..d - 1 {
            b = b.gfmul(ki[i + 1]) ^ ki[d + i]
        }

        self.state.exec_counter += 1;
        self.state.vope_counter += 1;

        Ok(b)
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
    /// In this state the sender performs VOPE extension.
    pub struct Extension {
        /// Sender's global secret.
        #[allow(dead_code)]
        pub(crate) delta: Block,

        /// Current VOPE counter
        pub(super) vope_counter: usize,
        /// Current execution counter
        pub(super) exec_counter: usize,
    }

    impl State for Extension {}

    opaque_debug::implement!(Extension);
}
