#![no_std]

// Use Soroban's allocator for heap allocations
extern crate alloc;

use soroban_sdk::{contract, contractimpl};

mod crypto;
mod test;

#[contract]
pub struct RiscZeroVerifier;

#[contractimpl]
impl RiscZeroVerifier {}
