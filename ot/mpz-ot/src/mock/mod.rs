//! Mock implementations of the OT protocols.

//mod owned;
mod shared;

//pub use owned::*;
pub use shared::{mock_ot_pair, MockSharedOTReceiver, MockSharedOTSender};
