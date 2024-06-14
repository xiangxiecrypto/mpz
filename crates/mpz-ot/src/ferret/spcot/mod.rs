//! Implementation of the Single-Point COT (spcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

mod error;
mod receiver;
mod sender;

pub(crate) use error::{ReceiverError, SenderError};
pub(crate) use receiver::Receiver;
pub(crate) use sender::Sender;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ideal::cot::{ideal_rcot, IdealCOTReceiver, IdealCOTSender},
        OTError,
    };
    use futures::TryFutureExt;
    use mpz_common::executor::test_st_executor;
    use mpz_core::Block;

    fn setup() -> (
        Sender<IdealCOTSender>,
        Receiver<IdealCOTReceiver>,
        IdealCOTSender,
        IdealCOTReceiver,
        Block,
    ) {
        let (mut rcot_sender, rcot_receiver) = ideal_rcot();

        let delta = rcot_sender.alice().get_mut().delta();

        let sender = Sender::new();
        let receiver = Receiver::new();

        (sender, receiver, rcot_sender, rcot_receiver, delta)
    }

    #[tokio::test]
    async fn test_spcot() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut sender, mut receiver, rcot_sender, rcot_receiver, delta) = setup();

        // shold set the same delta as in RCOT.
        sender.setup_with_delta(delta, rcot_sender).unwrap();
        receiver.setup(rcot_receiver).unwrap();

        let hs = [8, 4];
        let alphas = [4, 2];

        tokio::try_join!(
            sender.extend(&mut ctx_sender, &hs).map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, &hs)
                .map_err(OTError::from)
        )
        .unwrap();

        let (mut output_sender, output_receiver) = tokio::try_join!(
            sender.check(&mut ctx_sender).map_err(OTError::from),
            receiver.check(&mut ctx_receiver).map_err(OTError::from)
        )
        .unwrap();

        assert!(output_sender
            .iter_mut()
            .zip(output_receiver.iter())
            .all(|(vs, (ws, alpha))| {
                vs[*alpha as usize] ^= delta;
                vs == ws
            }));

        // extend twice.
        let hs = [6, 9, 8];
        let alphas = [2, 1, 3];

        tokio::try_join!(
            sender.extend(&mut ctx_sender, &hs).map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, &hs)
                .map_err(OTError::from)
        )
        .unwrap();

        let (mut output_sender, output_receiver) = tokio::try_join!(
            sender.check(&mut ctx_sender).map_err(OTError::from),
            receiver.check(&mut ctx_receiver).map_err(OTError::from)
        )
        .unwrap();

        assert!(output_sender
            .iter_mut()
            .zip(output_receiver.iter())
            .all(|(vs, (ws, alpha))| {
                vs[*alpha as usize] ^= delta;
                vs == ws
            }));

        sender.finalize().unwrap();
        receiver.finalize().unwrap();
    }
}
