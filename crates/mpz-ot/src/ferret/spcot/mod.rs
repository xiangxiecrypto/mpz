//! Implementation of the Single-Point COT (spcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

mod error;
mod receiver;
mod sender;

pub use error::{ReceiverError, SenderError};
pub use receiver::Receiver;
pub use sender::Sender;

#[cfg(test)]
mod tests {
    use crate::{
        ideal::cot::{ideal_rcot, IdealCOTReceiver, IdealCOTSender},
        COTSender, OTError, RandomCOTReceiver,
    };

    use super::*;
    use futures::TryFutureExt;
    use mpz_common::{executor::test_st_executor, Context};
    use mpz_core::{prg::Prg, Block};
    use mpz_ot_core::{COTSenderOutput, RCOTReceiverOutput};

    fn setup() -> (Sender<IdealCOTSender>, Receiver<IdealCOTReceiver>) {
        let (rcot_sender, rcot_receiver) = ideal_rcot();

        let sender = Sender::new(rcot_sender);
        let receiver = Receiver::new(rcot_receiver);

        (sender, receiver)
    }

    async fn rcot_sender_test<RandomCOT, Ctx>(
        rcot: &mut RandomCOT,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<(), SenderError>
    where
        Ctx: Context,
        RandomCOT: COTSender<Ctx, Block>,
    {
        let COTSenderOutput { msgs: _qs, .. } = rcot.send_correlated(ctx, count).await.unwrap();

        Ok(())
    }

    async fn rcot_receiver_test<RandomCOT, Ctx>(
        rcot: &mut RandomCOT,
        ctx: &mut Ctx,
        count: usize,
    ) -> Result<(), ReceiverError>
    where
        Ctx: Context,
        RandomCOT: RandomCOTReceiver<Ctx, bool, Block>,
    {
        let RCOTReceiverOutput {
            choices: _rs,
            msgs: _ts,
            ..
        } = rcot.receive_random_correlated(ctx, count).await.unwrap();

        Ok(())
    }

    #[tokio::test]
    async fn test_rcot() {
        let (mut rcot_sender, mut rcot_receiver) = ideal_rcot();

        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);
        let count = 10;

        tokio::try_join!(
            rcot_sender_test(&mut rcot_sender, &mut ctx_sender, count).map_err(OTError::from),
            rcot_receiver_test(&mut rcot_receiver, &mut ctx_receiver, count).map_err(OTError::from)
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_spcot() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut sender, mut receiver) = setup();

        let delta = Prg::new().random_block();
        let seed = Prg::new().random_block();

        sender.setup_with_delta(delta, seed).unwrap();
        receiver.setup().unwrap();

        let count = 1 << 8;
        let alpha = 3;

        tokio::try_join!(
            sender.extend(&mut ctx_sender, count).map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, alpha, count)
                .map_err(OTError::from)
        )
        .unwrap();

        // let count = 1 << 4;
        // let alpha = 2;

        // tokio::try_join!(
        //     sender.extend(&mut ctx_sender, count).map_err(OTError::from),
        //     receiver
        //         .extend(&mut ctx_receiver, alpha, count)
        //         .map_err(OTError::from)
        // )
        // .unwrap();

        // let (output_sender, output_receiver) = tokio::try_join!(
        //     sender.check(&mut ctx_sender).map_err(OTError::from),
        //     receiver.check(&mut ctx_receiver).map_err(OTError::from)
        // )
        // .unwrap();
    }
}
