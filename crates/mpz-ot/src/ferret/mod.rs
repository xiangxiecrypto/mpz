//! An implementation of the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) protocol.
mod error;
mod mpcot;
mod receiver;
mod sender;
mod spcot;

pub use error::{ReceiverError, SenderError};
pub use receiver::Receiver;
pub use sender::Sender;

use mpz_core::lpn::LpnParameters;
use mpz_ot_core::ferret::LpnType;

/// Configuration of Ferret.
#[derive(Debug)]
pub struct FerretConfig<RandomCOT, SetupRandomCOT> {
    rcot: RandomCOT,
    setup_rcot: SetupRandomCOT,
    lpn_parameters: LpnParameters,
    lpn_type: LpnType,
}

impl<RandomCOT: Clone, SetupRandomCOT> FerretConfig<RandomCOT, SetupRandomCOT> {
    /// Create a new instance.
    ///
    /// # Arguments.
    ///
    /// * `rcot` - The rcot for MPCOT.
    /// * `setup_rcot` - The rcot for setup.
    /// * `lpn_parameters` - The parameters of LPN.
    /// * `lpn_type` - The type of LPN.
    pub fn new(
        rcot: RandomCOT,
        setup_rcot: SetupRandomCOT,
        lpn_parameters: LpnParameters,
        lpn_type: LpnType,
    ) -> Self {
        Self {
            rcot,
            setup_rcot,
            lpn_parameters,
            lpn_type,
        }
    }

    /// Get rcot
    pub fn rcot(&self) -> RandomCOT {
        self.rcot.clone()
    }

    /// Get the setup rcot
    pub fn setup_rcot(&mut self) -> &mut SetupRandomCOT {
        &mut self.setup_rcot
    }

    /// Get the lpn type
    pub fn lpn_type(&self) -> LpnType {
        self.lpn_type
    }

    /// Get the lpn parameters
    pub fn lpn_parameters(&self) -> LpnParameters {
        self.lpn_parameters
    }
}

#[cfg(test)]
mod tests {
    use futures::TryFutureExt;
    use mpz_common::executor::test_st_executor;
    use mpz_core::{lpn::LpnParameters, Block};
    use mpz_ot_core::{ferret::LpnType, test::assert_cot, RCOTReceiverOutput, RCOTSenderOutput};

    use crate::{
        ideal::cot::{ideal_rcot, IdealCOTReceiver, IdealCOTSender},
        OTError, RandomCOTReceiver, RandomCOTSender,
    };

    use super::*;

    // l = n - k = 8380
    const LPN_PARAMETERS_TEST: LpnParameters = LpnParameters {
        n: 9600,
        k: 1220,
        t: 600,
    };

    fn setup() -> (
        Sender<IdealCOTSender, IdealCOTSender>,
        Receiver<IdealCOTReceiver, IdealCOTReceiver>,
        Block,
    ) {
        let (mut rcot_sender, rcot_receiver) = ideal_rcot();

        let sender_config = FerretConfig::new(
            rcot_sender.clone(),
            rcot_sender.clone(),
            LPN_PARAMETERS_TEST,
            LpnType::Regular,
        );

        let receiver_config = FerretConfig::new(
            rcot_receiver.clone(),
            rcot_receiver,
            LPN_PARAMETERS_TEST,
            LpnType::Regular,
        );

        let delta = rcot_sender.alice().get_mut().delta();

        let sender = Sender::new(sender_config);

        let receiver = Receiver::new(receiver_config);

        (sender, receiver, delta)
    }

    #[tokio::test]
    async fn test_ferret() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut sender, mut receiver, delta) = setup();

        tokio::try_join!(
            sender
                .setup_with_delta(&mut ctx_sender, delta)
                .map_err(OTError::from),
            receiver.setup(&mut ctx_receiver).map_err(OTError::from)
        )
        .unwrap();

        // extend once.
        let count = 8000;
        let (
            RCOTSenderOutput {
                id: sender_id,
                msgs: u,
            },
            RCOTReceiverOutput {
                id: receiver_id,
                choices: b,
                msgs: w,
            },
        ) = tokio::try_join!(
            sender.send_random_correlated(&mut ctx_sender, count),
            receiver.receive_random_correlated(&mut ctx_receiver, count)
        )
        .unwrap();

        assert_eq!(sender_id, receiver_id);
        assert_cot(delta, &b, &u, &w);

        // extend twice
        let count = 9000;
        let (
            RCOTSenderOutput {
                id: sender_id,
                msgs: u,
            },
            RCOTReceiverOutput {
                id: receiver_id,
                choices: b,
                msgs: w,
            },
        ) = tokio::try_join!(
            sender.send_random_correlated(&mut ctx_sender, count),
            receiver.receive_random_correlated(&mut ctx_receiver, count)
        )
        .unwrap();

        assert_eq!(sender_id, receiver_id);
        assert_cot(delta, &b, &u, &w);
    }
}
