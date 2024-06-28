//! Implementation of the Single-Point COT (spcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

pub mod error;
pub mod msgs;
pub mod receiver;
pub mod sender;

#[cfg(test)]
mod tests {
    use mpz_core::prg::Prg;

    use super::{receiver::Receiver as SpcotReceiver, sender::Sender as SpcotSender};
    use crate::{ferret::CSP, ideal::cot::IdealCOT, RCOTReceiverOutput, RCOTSenderOutput};

    #[test]
    fn spcot_test() {
        let mut ideal_cot = IdealCOT::default();
        let sender = SpcotSender::new();
        let receiver = SpcotReceiver::new();

        let mut prg = Prg::new();
        let sender_seed = prg.random_block();
        let delta = ideal_cot.delta();

        let mut sender = sender.setup(delta, sender_seed);
        let mut receiver = receiver.setup();

        let h1 = 8;
        let alpha1 = 3;

        // Extend once
        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(h1);

        let RCOTReceiverOutput {
            choices: rs,
            msgs: ts,
            ..
        } = msg_for_receiver;
        let RCOTSenderOutput { msgs: qs, .. } = msg_for_sender;
        let maskbits = receiver.extend_mask_bits(h1, alpha1, &rs).unwrap();

        let msg_from_sender = sender.extend(h1, &qs, maskbits).unwrap();

        receiver.extend(h1, alpha1, &ts, msg_from_sender).unwrap();

        // Extend twice
        let h2 = 4;
        let alpha2 = 2;

        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(h2);

        let RCOTReceiverOutput {
            choices: rs,
            msgs: ts,
            ..
        } = msg_for_receiver;
        let RCOTSenderOutput { msgs: qs, .. } = msg_for_sender;

        let maskbits = receiver.extend_mask_bits(h2, alpha2, &rs).unwrap();

        let msg_from_sender = sender.extend(h2, &qs, maskbits).unwrap();

        receiver.extend(h2, alpha2, &ts, msg_from_sender).unwrap();

        // Check
        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(CSP);

        let RCOTReceiverOutput {
            choices: x_star,
            msgs: z_star,
            ..
        } = msg_for_receiver;

        let RCOTSenderOutput { msgs: y_star, .. } = msg_for_sender;

        let check_from_receiver = receiver.check_pre(&x_star).unwrap();

        let (mut output_sender, check) = sender.check(&y_star, check_from_receiver).unwrap();

        let output_receiver = receiver.check(&z_star, check).unwrap();

        assert!(output_sender
            .iter_mut()
            .zip(output_receiver.iter())
            .all(|(vs, (ws, alpha))| {
                vs[*alpha as usize] ^= delta;
                vs == ws
            }));
    }
}

//! Implementation of the Single-Point COT (spcot) protocol in the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) paper.

pub mod error;
pub mod msgs;
pub mod receiver;
pub mod sender;

#[cfg(test)]
mod tests {
    use super::{receiver::Receiver as SpcotReceiver, sender::Sender as SpcotSender};
    use crate::{ferret::CSP, ideal::cot::IdealCOT, RCOTReceiverOutput, RCOTSenderOutput};

    #[test]
    fn spcot_test() {
        let mut ideal_cot = IdealCOT::default();
        let sender = SpcotSender::new();
        let receiver = SpcotReceiver::new();

        let delta = ideal_cot.delta();

        let mut sender = sender.setup(delta);
        let mut receiver = receiver.setup();

        let hs = [8, 4, 10];
        let alphas = [3, 2, 4];

        let h_sum = hs.iter().sum();
        // batch extension
        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(h_sum);

        let RCOTReceiverOutput {
            choices: rss,
            msgs: tss,
            ..
        } = msg_for_receiver;

        let RCOTSenderOutput { msgs: qss, .. } = msg_for_sender;

        let maskbits = receiver.extend_mask_bits(&hs, &alphas, &rss).unwrap();

        let msg_from_sender = sender.extend(&hs, &qss, &maskbits).unwrap();

        receiver
            .extend(&hs, &alphas, &tss, &msg_from_sender)
            .unwrap();

        // Check
        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(CSP);

        let RCOTReceiverOutput {
            choices: x_star,
            msgs: z_star,
            ..
        } = msg_for_receiver;

        let RCOTSenderOutput { msgs: y_star, .. } = msg_for_sender;

        let check_from_receiver = receiver.check_pre(&x_star).unwrap();

        let (mut output_sender, check) = sender.check(&y_star, check_from_receiver).unwrap();

        let output_receiver = receiver.check(&z_star, check).unwrap();

        assert!(output_sender
            .iter_mut()
            .zip(output_receiver.iter())
            .all(|(vs, (ws, alpha))| {
                vs[*alpha as usize] ^= delta;
                vs == ws
            }));

        // extend twice
        let hs = [6, 9, 8];
        let alphas = [2, 1, 3];

        let h_sum = hs.iter().sum();

        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(h_sum);

        let RCOTReceiverOutput {
            choices: rss,
            msgs: tss,
            ..
        } = msg_for_receiver;

        let RCOTSenderOutput { msgs: qss, .. } = msg_for_sender;

        let maskbits = receiver.extend_mask_bits(&hs, &alphas, &rss).unwrap();

        let msg_from_sender = sender.extend(&hs, &qss, &maskbits).unwrap();

        receiver
            .extend(&hs, &alphas, &tss, &msg_from_sender)
            .unwrap();

        // Check
        let (msg_for_sender, msg_for_receiver) = ideal_cot.random_correlated(CSP);

        let RCOTReceiverOutput {
            choices: x_star,
            msgs: z_star,
            ..
        } = msg_for_receiver;

        let RCOTSenderOutput { msgs: y_star, .. } = msg_for_sender;

        let check_from_receiver = receiver.check_pre(&x_star).unwrap();

        let (mut output_sender, check) = sender.check(&y_star, check_from_receiver).unwrap();

        let output_receiver = receiver.check(&z_star, check).unwrap();

        assert!(output_sender
            .iter_mut()
            .zip(output_receiver.iter())
            .all(|(vs, (ws, alpha))| {
                vs[*alpha as usize] ^= delta;
                vs == ws
            }));
    }
}
