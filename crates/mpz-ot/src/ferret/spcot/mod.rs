//! Implementation of the Single-Point COT (spcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

mod error;
mod receiver;
mod sender;

pub use error::{ReceiverError, SenderError};
pub use receiver::Receiver;
pub use sender::Sender;

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

    fn setup() -> (Sender<IdealCOTSender>, Receiver<IdealCOTReceiver>, Block) {
        let (mut rcot_sender, rcot_receiver) = ideal_rcot();

        let delta = rcot_sender.0.get_mut().delta();

        let sender = Sender::new(rcot_sender);
        let receiver = Receiver::new(rcot_receiver);

        (sender, receiver, delta)
    }

    #[tokio::test]
    async fn test_spcot() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut sender, mut receiver, delta) = setup();

        // shold set the same delta as in RCOT.
        sender.setup_with_delta(delta).unwrap();
        receiver.setup().unwrap();

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
    }
}
