use crate::{chou_orlandi::SenderError, OTError, OTSender, VerifyChoices};

use async_trait::async_trait;
use futures_util::SinkExt;
use mpz_core::{Block, ProtocolMessage};
use mpz_ot_core::chou_orlandi::{
    msgs::Message, sender_state as state, Sender as SenderCore, SenderConfig,
};
use utils_aio::{
    duplex::Duplex,
    non_blocking_backend::{Backend, NonBlockingBackend},
};

use enum_try_as_inner::EnumTryAsInner;

#[derive(Debug, EnumTryAsInner)]
enum State {
    Initialized(SenderCore<state::Initialized>),
    Setup(SenderCore<state::Setup>),
    Complete,
    Error,
}

impl From<enum_try_as_inner::Error<State>> for SenderError {
    fn from(value: enum_try_as_inner::Error<State>) -> Self {
        SenderError::StateError(value.to_string())
    }
}

/// Chou-Orlandi sender.
#[derive(Debug)]
pub struct Sender {
    state: State,
}

impl Sender {
    /// Creates a new Sender
    ///
    /// # Arguments
    ///
    /// * `config` - The Sender's configuration
    pub fn new(config: SenderConfig) -> Self {
        Self {
            state: State::Initialized(SenderCore::new(config)),
        }
    }

    /// Creates a new Sender with the provided RNG seed
    ///
    /// # Arguments
    ///
    /// * `config` - The Sender's configuration
    /// * `seed` - The RNG seed
    pub fn new_with_seed(config: SenderConfig, seed: [u8; 32]) -> Self {
        Self {
            state: State::Initialized(SenderCore::new_with_seed(config, seed)),
        }
    }

    /// Setups up the Sender.
    ///
    /// # Arguments
    ///
    /// * `channel` - The channel to communicate with the receiver.
    pub async fn setup<C: Duplex<Message>>(&mut self, channel: &mut C) -> Result<(), SenderError> {
        let sender = self.state.replace(State::Error).into_initialized()?;

        let (msg, sender) = sender.setup();

        channel.send(Message::SenderSetup(msg)).await?;

        let receiver_setup = channel.expect_next().await?.into_receiver_setup()?;

        let sender = Backend::spawn(|| sender.receive_setup(receiver_setup)).await?;

        self.state = State::Setup(sender);

        Ok(())
    }
}

impl ProtocolMessage for Sender {
    type Msg = Message;
}

#[async_trait]
impl OTSender<[Block; 2]> for Sender {
    async fn send<C: Duplex<Self::Msg>>(
        &mut self,
        channel: &mut C,
        input: &[[Block; 2]],
    ) -> Result<(), OTError> {
        let mut sender = self
            .state
            .replace(State::Error)
            .into_setup()
            .map_err(SenderError::from)?;

        let receiver_payload = channel
            .expect_next()
            .await?
            .into_receiver_payload()
            .map_err(SenderError::from)?;

        let input = input.to_vec();
        let (sender, payload) = Backend::spawn(move || {
            let payload = sender.send(&input, receiver_payload);
            (sender, payload)
        })
        .await;

        let payload = payload.map_err(SenderError::from)?;

        channel.send(Message::SenderPayload(payload)).await?;

        self.state = State::Setup(sender);

        Ok(())
    }
}

#[async_trait]
impl VerifyChoices<Vec<bool>> for Sender {
    async fn verify_choices<C: Duplex<Self::Msg>>(
        &mut self,
        channel: &mut C,
    ) -> Result<Vec<bool>, OTError> {
        let sender = self
            .state
            .replace(State::Complete)
            .into_setup()
            .map_err(SenderError::from)?;

        let receiver_reveal = channel
            .expect_next()
            .await?
            .into_receiver_reveal()
            .map_err(SenderError::from)?;

        Backend::spawn(move || sender.verify_choices(receiver_reveal))
            .await
            .map_err(SenderError::from)
            .map_err(OTError::from)
    }
}
