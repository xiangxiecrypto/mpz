use async_trait::async_trait;
use futures::SinkExt;
use itybity::BitIterable;
use mpz_core::{Block, ProtocolMessage};
use mpz_ot_core::chou_orlandi::{
    msgs::Message, receiver_state as state, Receiver as ReceiverCore, ReceiverConfig,
};

use enum_try_as_inner::EnumTryAsInner;
use utils_aio::{
    duplex::Duplex,
    non_blocking_backend::{Backend, NonBlockingBackend},
};

use crate::{OTError, OTReceiver, RevealChoices};

use super::ReceiverError;

#[derive(Debug, EnumTryAsInner)]
enum State {
    Initialized(Box<ReceiverCore<state::Initialized>>),
    Setup(Box<ReceiverCore<state::Setup>>),
    Complete,
    Error,
}

impl From<enum_try_as_inner::Error<State>> for ReceiverError {
    fn from(value: enum_try_as_inner::Error<State>) -> Self {
        ReceiverError::StateError(value.to_string())
    }
}

/// Chou-Orlandi receiver.
#[derive(Debug)]
pub struct Receiver {
    state: State,
}

impl Receiver {
    /// Creates a new receiver.
    ///
    /// # Arguments
    ///
    /// * `config` - The receiver's configuration
    pub fn new(config: ReceiverConfig) -> Self {
        Self {
            state: State::Initialized(Box::new(ReceiverCore::new(config))),
        }
    }

    /// Creates a new receiver with the provided RNG seed.
    ///
    /// # Arguments
    ///
    /// * `config` - The receiver's configuration
    pub fn new_with_seed(config: ReceiverConfig, seed: [u8; 32]) -> Self {
        Self {
            state: State::Initialized(Box::new(ReceiverCore::new_with_seed(config, seed))),
        }
    }

    /// Sets up the receiver.
    pub async fn setup<C: Duplex<Message>>(
        &mut self,
        channel: &mut C,
    ) -> Result<(), ReceiverError> {
        let receiver = self.state.replace(State::Error).into_initialized()?;

        let sender_setup = channel.expect_next().await?.into_sender_setup()?;

        let (receiver_setup, receiver) = Backend::spawn(move || receiver.setup(sender_setup)).await;

        channel.send(Message::ReceiverSetup(receiver_setup)).await?;

        self.state = State::Setup(Box::new(receiver));

        Ok(())
    }
}

impl ProtocolMessage for Receiver {
    type Msg = Message;
}

#[async_trait]
impl<T> OTReceiver<T, Block> for Receiver
where
    T: BitIterable + Send + Sync + Clone + 'static,
{
    async fn receive<C: Duplex<Self::Msg>>(
        &mut self,
        channel: &mut C,
        choices: &[T],
    ) -> Result<Vec<Block>, OTError> {
        let mut receiver = self
            .state
            .replace(State::Error)
            .into_setup()
            .map_err(ReceiverError::from)?;

        let choices = choices.to_vec();
        let (mut receiver, receiver_payload) = Backend::spawn(move || {
            let payload = receiver.receive_random(&choices);
            (receiver, payload)
        })
        .await;

        channel
            .send(Message::ReceiverPayload(receiver_payload))
            .await?;

        let sender_payload = channel
            .expect_next()
            .await?
            .into_sender_payload()
            .map_err(ReceiverError::from)?;

        let (receiver, data) = Backend::spawn(move || {
            let data = receiver.receive(sender_payload);
            (receiver, data)
        })
        .await;

        let data = data.map_err(ReceiverError::from)?;

        self.state = State::Setup(receiver);

        Ok(data)
    }
}

#[async_trait]
impl RevealChoices for Receiver {
    async fn reveal_choices<C: Duplex<Self::Msg>>(
        &mut self,
        channel: &mut C,
    ) -> Result<(), OTError> {
        let receiver = self
            .state
            .replace(State::Complete)
            .into_setup()
            .map_err(ReceiverError::from)?;

        let reveal = receiver.reveal_choices().map_err(ReceiverError::from)?;

        channel.send(Message::ReceiverReveal(reveal)).await?;

        Ok(())
    }
}
