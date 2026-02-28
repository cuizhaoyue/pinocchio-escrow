use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address, ProgramResult,
};
use pinocchio_token::{
    instructions::{CloseAccount, Transfer},
    state::TokenAccount,
};

use crate::{check_program, check_signer, close, init_ata_if_needed, verify_mint_account, Escrow};

pub struct RefundAccounts<'a> {
    pub maker: &'a AccountView,
    pub escrow: &'a AccountView,
    pub mint_a: &'a AccountView,
    pub vault: &'a AccountView,
    pub maker_ata_a: &'a AccountView,
    pub system_program: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for RefundAccounts<'a> {
    type Error = ProgramError;
    fn try_from(value: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [maker, escrow, mint_a, vault, maker_ata_a, system_program, token_program, ..] = value
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(Self {
            maker,
            escrow,
            mint_a,
            vault,
            maker_ata_a,
            system_program,
            token_program,
        })
    }
}

pub struct RefundContext<'a> {
    pub accounts: RefundAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountView]> for RefundContext<'a> {
    type Error = ProgramError;
    fn try_from(value: &'a [AccountView]) -> Result<Self, Self::Error> {
        let accounts = RefundAccounts::try_from(value)?;

        Ok(Self { accounts })
    }
}

impl<'a> RefundContext<'a> {
    pub const DISCRIMINATOR: &'a u8 = &2;
    pub fn process(&self, program_id: &Address) -> ProgramResult {
        // --------------- 通用参数校验 ------------------
        check_signer(self.accounts.maker)?;
        check_program(self.accounts.system_program, &pinocchio_system::ID)?;
        check_program(self.accounts.token_program, &pinocchio_token::ID)?;
        verify_mint_account(self.accounts.mint_a)?;
        if !self.accounts.escrow.owned_by(program_id) {
            return Err(ProgramError::InvalidAccountData);
        }
        init_ata_if_needed(
            self.accounts.maker_ata_a,
            self.accounts.mint_a,
            self.accounts.maker,
            self.accounts.maker,
            self.accounts.system_program,
            self.accounts.token_program,
        )?;

        // --------------- 获取托管信息 ------------------
        let (escorw_seed, escrow_bump) = {
            let escrow = Escrow::load(self.accounts.escrow)?;
            (escrow.seed(), escrow.bump())
        };

        // --------------- 创建PDA签名串 ------------------
        let seed_binding = escorw_seed.to_le_bytes();
        let escrow_seeds = [
            Seed::from(b"escrow"),
            Seed::from(self.accounts.maker.address().as_ref()),
            Seed::from(&seed_binding),
            Seed::from(&escrow_bump),
        ];
        let escrow_signer = Signer::from(&escrow_seeds);

        let vault = unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault)? };
        if vault.owner().ne(self.accounts.escrow.address())
            || vault.mint().ne(self.accounts.mint_a.address())
        {
            return Err(ProgramError::InvalidAccountData);
        }
        let amount = vault.amount();
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.maker_ata_a,
            authority: self.accounts.escrow,
            amount: amount,
        }
        .invoke_signed(&[escrow_signer.clone()])?;

        // --------------- 关闭金库账户和托管账户 ------------------
        CloseAccount {
            account: self.accounts.vault,
            destination: self.accounts.maker,
            authority: self.accounts.escrow,
        }
        .invoke_signed(&[escrow_signer.clone()])?;
        close(self.accounts.escrow, self.accounts.maker)?;

        Ok(())
    }
}
