//! An implementation of the Chou-Orlandi [`CO15`](https://eprint.iacr.org/2015/267.pdf) oblivious transfer protocol.
//!
//! # Examples
//!
//! ```
//! use utils_aio::duplex::MpscDuplex;
//! use mpz_ot::chou_orlandi::{Receiver, Sender, SenderConfig, ReceiverConfig};
//! use mpz_ot::{OTReceiver, OTSender};
//! use mpz_core::Block;
//!
//! # futures::executor::block_on(async {
//! // An in-memory duplex channel.
//! let (mut sender_channel, mut receiver_channel) = MpscDuplex::new();
//!
//! let mut sender = Sender::new(SenderConfig::default());
//! let mut receiver = Receiver::new(ReceiverConfig::default());
//!
//! // Perform the setup phase.
//! let (sender_res, receiver_res) = futures::join!(
//!     sender.setup(&mut sender_channel),
//!     receiver.setup(&mut receiver_channel)
//! );
//!
//! sender_res.unwrap();
//! receiver_res.unwrap();
//!
//! // Perform the transfer phase.
//! let messages = vec![[Block::new(0), Block::new(1)], [Block::new(2), Block::new(3)]];
//!
//! let (sender_res, receiver_res) = futures::join!(
//!     sender.send(&mut sender_channel, &messages),
//!     receiver.receive(&mut receiver_channel, &[true, false])
//! );
//!
//! sender_res.unwrap();
//!
//! let received = receiver_res.unwrap();
//!
//! assert_eq!(received, vec![Block::new(1), Block::new(2)]);
//! # });
//! ```
//!
//! # Committed Receiver
//!
//! This implementation also provides support for a committed receiver. This is a receiver that commits to their choice
//! bits, and can later provably reveal them to the sender.
//!
//! ## Example
//!
//! ```
//! use utils_aio::duplex::MpscDuplex;
//! use mpz_ot::chou_orlandi::{Receiver, Sender, SenderConfig, ReceiverConfig};
//! use mpz_ot::{OTReceiver, OTSender, RevealChoices, VerifyChoices};
//! use mpz_core::Block;
//!
//! # futures::executor::block_on(async {
//! // An in-memory duplex channel.
//! let (mut sender_channel, mut receiver_channel) = MpscDuplex::new();
//!
//! // Enable committed receiver in config.
//! let mut sender = Sender::new(SenderConfig::builder().receiver_commit().build().unwrap());
//! let mut receiver = Receiver::new(ReceiverConfig::builder().receiver_commit().build().unwrap());
//!
//! // Perform the setup phase.
//! let (sender_res, receiver_res) = futures::join!(
//!     sender.setup(&mut sender_channel),
//!     receiver.setup(&mut receiver_channel)
//! );
//!
//! sender_res.unwrap();
//! receiver_res.unwrap();
//!
//! // Perform the transfer phase.
//! let messages = vec![[Block::new(0), Block::new(1)], [Block::new(2), Block::new(3)]];
//!
//! let (sender_res, receiver_res) = futures::join!(
//!     sender.send(&mut sender_channel, &messages),
//!     receiver.receive(&mut receiver_channel, &[true, false])
//! );
//!
//! sender_res.unwrap();
//! _ = receiver_res.unwrap();
//!
//! // Reveal the choice bits.
//! let (sender_res, receiver_res) = futures::join!(
//!     sender.verify_choices(&mut sender_channel),
//!     receiver.reveal_choices(&mut receiver_channel)
//! );
//!
//! receiver_res.unwrap();
//!
//! // The verified choice bits are returned to the sender.
//! let choices = sender_res.unwrap();
//!
//! assert_eq!(choices, vec![true, false]);
//! # });
//! ```

mod error;
mod receiver;
mod sender;

pub use error::{ReceiverError, SenderError};
pub use receiver::Receiver;
pub use sender::Sender;

pub use mpz_ot_core::chou_orlandi::{
    msgs, ReceiverConfig, ReceiverConfigBuilder, ReceiverConfigBuilderError, SenderConfig,
    SenderConfigBuilder, SenderConfigBuilderError,
};

#[cfg(test)]
mod tests {
    use itybity::ToBits;
    use mpz_core::Block;
    use rand::Rng;
    use rand_chacha::ChaCha12Rng;
    use rand_core::SeedableRng;
    use utils_aio::duplex::{Duplex, MpscDuplex};

    use crate::{OTReceiver, OTSender, RevealChoices, VerifyChoices};

    use super::*;
    use rstest::*;

    #[fixture]
    fn choices() -> Vec<bool> {
        let mut rng = ChaCha12Rng::seed_from_u64(0);
        (0..128).map(|_| rng.gen()).collect()
    }

    #[fixture]
    fn data() -> Vec<[Block; 2]> {
        let mut rng = ChaCha12Rng::seed_from_u64(0);
        (0..128)
            .map(|_| [rng.gen::<[u8; 16]>().into(), rng.gen::<[u8; 16]>().into()])
            .collect()
    }

    fn choose<T>(
        data: impl Iterator<Item = [T; 2]>,
        choices: impl Iterator<Item = bool>,
    ) -> impl Iterator<Item = T> {
        data.zip(choices)
            .map(|([zero, one], choice)| if choice { one } else { zero })
    }

    async fn setup<C: Duplex<msgs::Message>>(
        sender_config: SenderConfig,
        receiver_config: ReceiverConfig,
        sender_channel: &mut C,
        receiver_channel: &mut C,
    ) -> (Sender, Receiver) {
        let mut sender = Sender::new(sender_config);
        let mut receiver = Receiver::new(receiver_config);

        let (sender_res, receiver_res) = tokio::join!(
            sender.setup(sender_channel),
            receiver.setup(receiver_channel)
        );

        sender_res.unwrap();
        receiver_res.unwrap();

        (sender, receiver)
    }

    #[rstest]
    #[tokio::test]
    async fn test_chou_orlandi(data: Vec<[Block; 2]>, choices: Vec<bool>) {
        let (mut sender_channel, mut receiver_channel) = MpscDuplex::new();

        let (mut sender, mut receiver) = setup(
            SenderConfig::default(),
            ReceiverConfig::default(),
            &mut sender_channel,
            &mut receiver_channel,
        )
        .await;

        let (sender_res, receiver_res) = tokio::join!(
            sender.send(&mut sender_channel, &data),
            receiver.receive(&mut receiver_channel, &choices)
        );

        sender_res.unwrap();
        let received = receiver_res.unwrap();

        let expected = choose(data.iter().copied(), choices.iter_lsb0()).collect::<Vec<_>>();

        assert_eq!(received, expected);
    }

    #[rstest]
    #[tokio::test]
    async fn test_chou_orlandi_committed_receiver(data: Vec<[Block; 2]>, choices: Vec<bool>) {
        let (mut sender_channel, mut receiver_channel) = MpscDuplex::new();

        let (mut sender, mut receiver) = setup(
            SenderConfig::builder().receiver_commit().build().unwrap(),
            ReceiverConfig::builder().receiver_commit().build().unwrap(),
            &mut sender_channel,
            &mut receiver_channel,
        )
        .await;

        let (sender_res, receiver_res) = tokio::join!(
            sender.send(&mut sender_channel, &data),
            receiver.receive(&mut receiver_channel, &choices)
        );

        sender_res.unwrap();
        _ = receiver_res.unwrap();

        let (sender_res, receiver_res) = tokio::join!(
            sender.verify_choices(&mut sender_channel),
            receiver.reveal_choices(&mut receiver_channel)
        );

        let verified_choices = sender_res.unwrap();
        receiver_res.unwrap();

        assert_eq!(verified_choices, choices);
    }
}
