//! Implementation of the Multiple-Point COT (mpcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

mod error;
mod receiver;
mod sender;

pub use error::{ReceiverError, SenderError};
pub use receiver::Receiver;
pub use sender::Sender;

#[cfg(test)]
mod tests {
    use futures::TryFutureExt;
    use mpz_common::executor::test_st_executor;
    use mpz_core::Block;

    use crate::{
        ideal::cot::{ideal_rcot, IdealCOTReceiver, IdealCOTSender},
        OTError,
    };

    use super::*;

    fn setup() -> (Sender<IdealCOTSender>, Receiver<IdealCOTReceiver>, Block) {
        let (mut rcot_sender, rcot_receiver) = ideal_rcot();

        let delta = rcot_sender.0.get_mut().delta();

        let sender = Sender::new(rcot_sender);

        let receiver = Receiver::new(rcot_receiver);

        (sender, receiver, delta)
    }

    #[tokio::test]
    async fn test_general_mpcot() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut sender, mut receiver, delta) = setup();

        let alphas = [0, 1, 3, 4, 2];
        let t = alphas.len();
        let n = 10;

        tokio::try_join!(
            sender
                .setup_with_delta(&mut ctx_sender, delta)
                .map_err(OTError::from),
            receiver.setup(&mut ctx_receiver).map_err(OTError::from)
        )
        .unwrap();

        tokio::try_join!(
            sender
                .extend(&mut ctx_sender, t as u32, n)
                .map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, n)
                .map_err(OTError::from)
        )
        .unwrap();

        sender.finalize().unwrap();
        receiver.finalize().unwrap();
    }
}
