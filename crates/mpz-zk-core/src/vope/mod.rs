//! This is the implementation of vector oblivious polynomial evaluation (VOPE) based on Figure 4 in https://eprint.iacr.org/2021/076.pdf

pub mod error;
pub mod receiver;
pub mod sender;

/// Security parameter
pub const CSP: usize = 128;

#[cfg(test)]
mod tests {
    use mpz_core::prg::Prg;
    use mpz_ot_core::{ideal::cot::IdealCOT, RCOTReceiverOutput, RCOTSenderOutput};

    use crate::test::poly_check;

    use super::{receiver::Receiver, sender::Sender, CSP};

    #[test]
    fn vope_test() {
        let mut prg = Prg::new();
        let delta = prg.random_block();

        let mut ideal_cot = IdealCOT::default();
        ideal_cot.set_delta(delta);

        let sender = Sender::new();
        let receiver = Receiver::new();

        let mut sender = sender.setup(delta);
        let mut receiver = receiver.setup();

        let d = 1;

        let (sender_cot, receiver_cot) = ideal_cot.random_correlated((2 * d - 1) * CSP);

        let RCOTSenderOutput { msgs: ks, .. } = sender_cot;
        let RCOTReceiverOutput {
            msgs: ms,
            choices: us,
            ..
        } = receiver_cot;

        let sender_out = sender.extend(&ks, d).unwrap();
        let receiver_out = receiver.extend(&ms, &us, d).unwrap();

        assert!(poly_check(&receiver_out, sender_out, delta));

        let d = 5;

        let (sender_cot, receiver_cot) = ideal_cot.random_correlated((2 * d - 1) * CSP);

        let RCOTSenderOutput { msgs: ks, .. } = sender_cot;
        let RCOTReceiverOutput {
            msgs: ms,
            choices: us,
            ..
        } = receiver_cot;

        let sender_out = sender.extend(&ks, d).unwrap();
        let receiver_out = receiver.extend(&ms, &us, d).unwrap();

        assert!(poly_check(&receiver_out, sender_out, delta));
    }
}
