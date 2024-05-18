//! An implementation of the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) protocol.
mod error;
mod receiver;
mod sender;

pub mod mpcot;
pub mod spcot;

pub use error::{ReceiverError, SenderError};
pub use receiver::Receiver;
pub use sender::Sender;

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
        IdealCOTSender,
        IdealCOTReceiver,
        Sender<IdealCOTSender>,
        Receiver<IdealCOTReceiver>,
        Block,
    ) {
        let (mut rcot_sender, rcot_receiver) = ideal_rcot();

        let delta = rcot_sender.alice().get_mut().delta();

        let sender = Sender::new(rcot_sender.clone());

        let receiver = Receiver::new(rcot_receiver.clone());

        (rcot_sender, rcot_receiver, sender, receiver, delta)
    }

    #[tokio::test]
    async fn test_ferret() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut rcot_sender, mut rcot_receiver, mut sender, mut receiver, delta) = setup();

        tokio::try_join!(
            sender
                .setup_with_parameters(
                    &mut ctx_sender,
                    &mut rcot_sender,
                    delta,
                    LPN_PARAMETERS_TEST,
                    // Can change the type to Unifrom
                    LpnType::Regular
                )
                .map_err(OTError::from),
            receiver
                .setup_with_parameters(
                    &mut ctx_receiver,
                    &mut rcot_receiver,
                    LPN_PARAMETERS_TEST,
                    // Can change the type to Unifrom
                    LpnType::Regular
                )
                .map_err(OTError::from)
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
