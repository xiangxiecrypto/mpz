//! test functions.

use mpz_core::Block;

/// Check polynomial relation.
pub fn poly_check(a: &[Block], b: Block, delta: Block) -> bool {
    b == a
        .iter()
        .rev()
        .fold(Block::ZERO, |acc, &x| x ^ (delta.gfmul(acc)))
}