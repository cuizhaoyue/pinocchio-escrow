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

declare_id!("22222222222222222222222222222222222222222222");

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    log!("Hello from my pinocchio program!");
    match instruction_data.split_first() {
        Some((MakeContext::DISCRIMINATOR, data)) => {
            MakeContext::try_from((accounts, data))?.process(program_id)
        }
        Some((TakeContext::DISCRIMINATOR, _)) => {
            TakeContext::try_from(accounts)?.process(program_id)
        }
        Some((RefundContext::DISCRIMINATOR, _)) => {
            RefundContext::try_from(accounts)?.process(program_id)
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
    // Ok(())
}
