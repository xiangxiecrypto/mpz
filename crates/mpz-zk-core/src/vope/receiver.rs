//! VOPE receiver.
use mpz_core::Block;

use crate::vope::CSP;

use super::error::ReceiverError;

/// VOPE receiver
/// This is the prover in Figure 4.
#[derive(Debug, Default)]
pub struct Receiver<T: state::State = state::Initialized> {
    state: T,
}

impl Receiver {
    /// Create a new receiver.
    pub fn new() -> Self {
        Receiver {
            state: state::Initialized::default(),
        }
    }

    /// Completes the setup phase of the protocol.
    ///
    /// See Initialize in Figure 4.
    pub fn setup(self) -> Receiver<state::Extension> {
        Receiver {
            state: state::Extension {
                vope_counter: 0,
                exec_counter: 0,
            },
        }
    }
}

impl Receiver<state::Extension> {
    /// Performs VOPE extension.
    ///
    /// See step 1-3 in Figure 4.
    ///
    /// # Arguments
    ///
    /// * `ms` - The blocks received by calling the COT ideal functionality.
    /// * `us` - The bits received by calling the COT ideal functionality.
    /// * `d` - The degree of the polynomial.
    ///
    /// Note that this functionality is only suitable for small d.
    pub fn extend(
        &mut self,
        ms: &[Block],
        us: &[bool],
        d: usize,
    ) -> Result<Vec<Block>, ReceiverError> {
        if d == 0 {
            return Err(ReceiverError::InvalidInput(
                "the degree d should not be 0".to_string(),
            ));
        }

        if ms.len() != us.len() {
            return Err(ReceiverError::InvalidLength(
                "the length of ms and us should be equal".to_string(),
            ));
        }

        if ms.len() != (2 * d - 1) * CSP {
            return Err(ReceiverError::InvalidLength(
                "the length of ms and us should be (2 * d -1) * CSP".to_string(),
            ));
        }

        let mut h_ms = ms.to_vec();
        let mut h_us = us.to_vec();

        let mut mi = vec![Block::ZERO; 2 * d - 1];
        let mut ui = vec![Block::ZERO; 2 * d - 1];

        let base: Vec<Block> = (0..CSP)
            .map(|x| bytemuck::cast((1_u128) << (CSP - 1 - x)))
            .collect();

        for i in 0..(2 * d - 1) {
            let m = h_ms.split_off(CSP);
            let u = h_us.split_off(CSP);

            mi[i] = Block::inn_prdt_red(&h_ms, &base);

            ui[i] =
                h_us.iter().zip(base.iter()).fold(
                    Block::ZERO,
                    |acc, (b, base)| {
                        if *b {
                            acc ^ *base
                        } else {
                            acc
                        }
                    },
                );
            h_ms = m;
            h_us = u;
        }

        let mut gi = vec![Block::ZERO; d + 1];
        gi[0] = mi[0];
        gi[1] = ui[0];

        for i in 0..d - 1 {
            poly_update(&mut gi, mi[i + 1], ui[i + 1], i + 2);
            gi[0] ^= mi[d + i];
            gi[1] ^= ui[d + i];
        }

        self.state.exec_counter += 1;
        self.state.vope_counter += 1;

        Ok(gi)
    }
}

fn poly_update(g: &mut [Block], m: Block, u: Block, length: usize) {
    let mut buffer = vec![Block::ZERO; length + 1];
    for i in 0..length {
        buffer[i + 1] = g[i].gfmul(u);
        g[i] = g[i].gfmul(m);

        g[i] ^= buffer[i];
    }
    g[length] = buffer[length];
}

/// The receiver's state.
pub mod state {
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
    /// In this state the sender performs VOPE extension.
    pub struct Extension {
        /// Current VOPE counter
        pub(super) vope_counter: usize,
        /// Current execution counter
        pub(super) exec_counter: usize,
    }

    impl State for Extension {}

    opaque_debug::implement!(Extension);
}
