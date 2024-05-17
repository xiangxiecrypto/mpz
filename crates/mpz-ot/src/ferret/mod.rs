//! An implementation of the [`Ferret`](https://eprint.iacr.org/2020/924.pdf) protocol.
mod sender;
mod error;

pub mod mpcot;
pub mod spcot;

pub use error::SenderError; 
pub use sender::Sender;
