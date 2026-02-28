use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address, ProgramResult,
};
use pinocchio_token::{
    instructions::{CloseAccount, Transfer},
    state::TokenAccount,
};
use solana_program_log::log;

use crate::{check_program, check_signer, close, init_ata_if_needed, verify_mint_account, Escrow};

pub struct TakeAccounts<'a> {
    pub taker: &'a AccountView,
    pub maker: &'a AccountView,
    pub escrow: &'a AccountView,
    pub mint_a: &'a AccountView,
    pub mint_b: &'a AccountView,
    pub vault: &'a AccountView,
    pub taker_ata_a: &'a AccountView,
    pub taker_ata_b: &'a AccountView,
    pub maker_ata_b: &'a AccountView,
    pub system_program: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for TakeAccounts<'a> {
    type Error = ProgramError;
    fn try_from(value: &'a [AccountView]) -> Result<Self, Self::Error> {
        if value.len() < 11 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        let taker = &value[0];
        let maker = &value[1];
        let escrow = &value[2];
        let mint_a = &value[3];
        let mint_b = &value[4];
        let vault = &value[5];
        let taker_ata_a = &value[6];
        let taker_ata_b = &value[7];
        let maker_ata_b = &value[8];
        let system_program = &value[9];
        let token_program = &value[10];

        Ok(Self {
            taker,
            maker,
            escrow,
            mint_a,
            mint_b,
            vault,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            system_program,
            token_program,
        })
    }
}

pub struct TakeContext<'a> {
    pub accounts: TakeAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountView]> for TakeContext<'a> {
    type Error = ProgramError;
    fn try_from(value: &'a [AccountView]) -> Result<Self, Self::Error> {
        let accounts = TakeAccounts::try_from(value)?;

        Ok(Self { accounts })
    }
}

impl<'a> TakeContext<'a> {
    pub const DISCRIMINATOR: &'a u8 = &1;

    pub fn process(&self, program_id: &Address) -> ProgramResult {
        // ------------- 通用参数检查 ------------------------
        check_signer(self.accounts.taker)?;
        check_program(self.accounts.system_program, &pinocchio_system::ID)?;
        check_program(self.accounts.token_program, &pinocchio_token::ID)?;
        verify_mint_account(self.accounts.mint_a)?;
        verify_mint_account(self.accounts.mint_b)?;
        if !self.accounts.escrow.owned_by(program_id) {
            return Err(ProgramError::InvalidAccountData);
        }
        init_ata_if_needed(
            self.accounts.taker_ata_a,
            self.accounts.mint_a,
            self.accounts.taker,
            self.accounts.taker,
            self.accounts.system_program,
            self.accounts.token_program,
        )?;
        init_ata_if_needed(
            self.accounts.maker_ata_b,
            self.accounts.mint_b,
            self.accounts.taker,
            self.accounts.maker,
            self.accounts.system_program,
            self.accounts.token_program,
        )?;

        // ------------- 获取订单数据 --------------------
        let (escrow_seed, escrow_receive, escrow_bump) = {
            let escrow = Escrow::load(self.accounts.escrow)?;
            (escrow.seed(), escrow.receive(), escrow.bump())
        };

        // -------------获取 Escrow PDA 签名 -------------------
        let seed_binding = escrow_seed.to_le_bytes();
        let escrow_seeds = [
            Seed::from(b"escrow"),
            Seed::from(self.accounts.maker.address().as_ref()),
            Seed::from(&seed_binding),
            Seed::from(&escrow_bump),
        ];
        let escrow_signers = [Signer::from(&escrow_seeds)];

        let vault = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault)? };
        if vault.owner().ne(self.accounts.escrow.address())
            || vault.mint().ne(self.accounts.mint_a.address())
        {
            return Err(ProgramError::InvalidAccountData);
        }

        let amount = vault.amount();

        // ------------- 转账 --------------------
        log("transfer 1");
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.taker_ata_a,
            authority: self.accounts.escrow,
            amount: amount,
        }
        .invoke_signed(&escrow_signers)?;

        log("close 1");
        CloseAccount {
            account: self.accounts.vault,
            authority: self.accounts.escrow,
            destination: self.accounts.maker,
        }
        .invoke_signed(&escrow_signers)?;

        log("transfer 2");
        Transfer {
            from: self.accounts.taker_ata_b,
            to: self.accounts.maker_ata_b,
            authority: self.accounts.taker,
            amount: escrow_receive,
        }
        .invoke()?;

        close(self.accounts.escrow, self.accounts.taker)?;

        Ok(())
    }
}
