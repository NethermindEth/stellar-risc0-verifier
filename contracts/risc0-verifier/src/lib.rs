#![no_std]
use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct RiscZeroVerifier;

#[contractimpl]
impl RiscZeroVerifier {}

mod test;
