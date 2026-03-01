#![allow(unexpected_cfgs)]

use pinocchio::{entrypoint, error::ProgramError, AccountView, Address, ProgramResult};
use solana_address::declare_id;
use solana_program_log::log;

mod state;
pub use state::*;

mod errors;
pub use errors::*;

mod instructions;
pub use instructions::*;

#[cfg(test)]
pub mod tests;

declare_id!("22222222222222222222222222222222222222222222");

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    log!("Hello from my pinocchio program!");
    match instruction_data.split_first() {
        Some((&MakeContext::DISCRIMINATOR, data)) => {
            MakeContext::try_from((data, accounts))?.process()
        }
        Some((&TakeContext::DISCRIMINATOR, _)) => TakeContext::try_from(accounts)?.process(),
        Some((&RefundContext::DISCRIMINATOR, _)) => RefundContext::try_from(accounts)?.process(),
        _ => Err(ProgramError::InvalidInstructionData),
    }
    // Ok(())
}
