//! Low-level crate containing core functionalities for zero-knowledge protocols.
//!
//! This crate is not intended to be used directly. Instead, use the higher-level APIs provided by
//! the `mpz-zk` crate.
//!
//! # ⚠️ Warning ⚠️
//!
//! Some implementations make assumptions about invariants which may not be checked if using these
//! low-level APIs naively. Failing to uphold these invariants may result in security vulnerabilities.
//!
//! USE AT YOUR OWN RISK.

#![deny(
    unsafe_code,
    missing_docs,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all
)]

pub mod vope;
