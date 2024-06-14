//! Implementation of the Multiple-Point COT (mpcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

mod error;
mod receiver;
mod sender;

pub(crate) use error::{ReceiverError, SenderError};
pub(crate) use receiver::Receiver;
pub(crate) use sender::Sender;

#[cfg(test)]
mod tests {
    use futures::TryFutureExt;
    use mpz_common::executor::test_st_executor;
    use mpz_core::Block;
    use mpz_ot_core::ferret::LpnType;

    use crate::{
        ideal::cot::{ideal_rcot, IdealCOTReceiver, IdealCOTSender},
        OTError,
    };

    use receiver::Receiver;
    use sender::Sender;

    use super::*;

    fn setup(
        lpn_type: LpnType,
    ) -> (
        Sender<IdealCOTSender>,
        Receiver<IdealCOTReceiver>,
        IdealCOTSender,
        IdealCOTReceiver,
        Block,
    ) {
        let (mut rcot_sender, rcot_receiver) = ideal_rcot();

        let delta = rcot_sender.alice().get_mut().delta();

        let sender = Sender::new(lpn_type);

        let receiver = Receiver::new(lpn_type);

        (sender, receiver, rcot_sender, rcot_receiver, delta)
    }

    #[tokio::test]
    async fn test_mpcot() {
        let (mut ctx_sender, mut ctx_receiver) = test_st_executor(8);

        let (mut sender, mut receiver, rcot_sender, rcot_receiver, delta) = setup(LpnType::Uniform);

        let alphas = [0, 1, 3, 4, 2];
        let t = alphas.len();
        let n = 10;

        tokio::try_join!(
            sender
                .setup_with_delta(&mut ctx_sender, delta, rcot_sender)
                .map_err(OTError::from),
            receiver
                .setup(&mut ctx_receiver, rcot_receiver)
                .map_err(OTError::from)
        )
        .unwrap();

        let (mut output_sender, output_receiver) = tokio::try_join!(
            sender
                .extend(&mut ctx_sender, t as u32, n)
                .map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, n)
                .map_err(OTError::from)
        )
        .unwrap();

        for i in alphas {
            output_sender[i as usize] ^= delta;
        }

        assert_eq!(output_sender, output_receiver);

        // extend twice
        let alphas = [5, 1, 7, 2];
        let t = alphas.len();
        let n = 16;

        let (mut output_sender, output_receiver) = tokio::try_join!(
            sender
                .extend(&mut ctx_sender, t as u32, n)
                .map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, n)
                .map_err(OTError::from)
        )
        .unwrap();

        for i in alphas {
            output_sender[i as usize] ^= delta;
        }

        assert_eq!(output_sender, output_receiver);

        sender.finalize().unwrap();
        receiver.finalize().unwrap();

        let (mut sender, mut receiver, rcot_sender, rcot_receiver, delta) = setup(LpnType::Regular);

        // extend once.
        let alphas = [0, 3, 4, 7, 9];
        let t = alphas.len();
        let n = 10;

        tokio::try_join!(
            sender
                .setup_with_delta(&mut ctx_sender, delta, rcot_sender)
                .map_err(OTError::from),
            receiver
                .setup(&mut ctx_receiver, rcot_receiver)
                .map_err(OTError::from)
        )
        .unwrap();

        let (mut output_sender, output_receiver) = tokio::try_join!(
            sender
                .extend(&mut ctx_sender, t as u32, n)
                .map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, n)
                .map_err(OTError::from)
        )
        .unwrap();

        for i in alphas {
            output_sender[i as usize] ^= delta;
        }

        assert_eq!(output_sender, output_receiver);

        // extend twice.
        let alphas = [0, 3, 7, 9, 14, 15];
        let t = alphas.len();
        let n = 16;

        let (mut output_sender, output_receiver) = tokio::try_join!(
            sender
                .extend(&mut ctx_sender, t as u32, n)
                .map_err(OTError::from),
            receiver
                .extend(&mut ctx_receiver, &alphas, n)
                .map_err(OTError::from)
        )
        .unwrap();

        for i in alphas {
            output_sender[i as usize] ^= delta;
        }

        assert_eq!(output_sender, output_receiver);

        sender.finalize().unwrap();
        receiver.finalize().unwrap();
    }
}
