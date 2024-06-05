//! This is the implementation of vector oblivious polynomial evaluation (VOPE) based on Figure 4 in https://eprint.iacr.org/2021/076.pdf

pub mod error;
pub mod msgs;
pub mod receiver;
pub mod sender;

/// Security parameter
pub const CSP: usize = 128;
