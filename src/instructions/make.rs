use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address, ProgramResult,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_system::create_account_with_minimum_balance_signed;
use pinocchio_token::instructions::Transfer;

use crate::{check_non_zero, check_program, check_signer, verify_mint_account, Escrow};

pub struct MakeAccounts<'a> {
    pub maker: &'a AccountView,
    pub escrow: &'a AccountView,
    pub mint_a: &'a AccountView,
    pub mint_b: &'a AccountView,
    pub maker_ata_a: &'a AccountView,
    pub vault: &'a AccountView,
    pub system_program: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for MakeAccounts<'a> {
    type Error = ProgramError;
    fn try_from(value: &'a [AccountView]) -> Result<Self, Self::Error> {
        if value.len() < 8 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let maker = &value[0];
        let escrow = &value[1];
        let mint_a = &value[2];
        let mint_b = &value[3];
        let maker_ata_a = &value[4];
        let vault = &value[5];
        let system_program = &value[6];
        let token_program = &value[7];

        Ok(Self {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ata_a,
            vault,
            system_program,
            token_program,
        })
    }
}

pub struct MakeInstructionData {
    pub seed: u64,    // 在种子派生过程中使用的随机数。必须是 u64
    pub receive: u64, // 创建者希望接收的金额。必须是 u64
    pub amount: u64,  // 创建者希望存入的金额。必须是 u64
}

impl TryFrom<&[u8]> for MakeInstructionData {
    type Error = ProgramError;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 24 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let seed = u64::from_le_bytes(
            value[0..8]
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );
        let receive = u64::from_le_bytes(
            value[8..16]
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );
        let amount = u64::from_le_bytes(
            value[16..24]
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );

        Ok(Self {
            seed,
            receive,
            amount,
        })
    }
}

pub struct MakeContext<'a> {
    pub accounts: MakeAccounts<'a>,
    pub instruction_data: MakeInstructionData,
}

impl<'a> TryFrom<(&'a [AccountView], &'a [u8])> for MakeContext<'a> {
    type Error = ProgramError;
    fn try_from(value: (&'a [AccountView], &'a [u8])) -> Result<Self, Self::Error> {
        let accounts = MakeAccounts::try_from(value.0)?;
        let instruction_data = MakeInstructionData::try_from(value.1)?;

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> MakeContext<'a> {
    pub const DISCRIMINATOR: &'a u8 = &0;
    pub fn process(&self, program_id: &Address) -> ProgramResult {
        // -------------- 通用参数检查 ---------------------
        check_signer(self.accounts.maker)?;
        check_program(self.accounts.token_program, &pinocchio_token::ID)?;
        check_non_zero(&[self.instruction_data.receive, self.instruction_data.amount])?;
        verify_mint_account(self.accounts.mint_a)?;
        verify_mint_account(self.accounts.mint_b)?;

        // -------------- 计算 Escrow PDA 账户创建时需要的bump值----------------
        let seed_binding = self.instruction_data.seed.to_le_bytes();
        let (escrow_pda, escrow_bump) = Address::find_program_address(
            &[
                b"escrow",
                self.accounts.maker.address().as_ref(),
                &seed_binding,
            ],
            program_id,
        );
        // 校验 PDA 账户是否匹配
        if &escrow_pda != self.accounts.escrow.address() {
            return Err(ProgramError::InvalidAccountData);
        }
        // -------------- 创建 Escrow PDA 签名 ---------------
        let escorw_bump_binding = [escrow_bump];
        let escrow_seeds = [
            Seed::from(b"escrow"),
            Seed::from(self.accounts.maker.address().as_ref()),
            Seed::from(&seed_binding),
            Seed::from(&escorw_bump_binding),
        ];
        let escrow_signers = Signer::from(&escrow_seeds);

        // -------------- 创建 Escrow PDA 账户 ---------------
        create_account_with_minimum_balance_signed(
            self.accounts.escrow,
            Escrow::LEN,
            program_id,
            self.accounts.maker,
            None,
            &[escrow_signers],
        )?;

        // --------------- 初始化Escrow数据 ------------------
        let mut escrow = Escrow::load_mut(self.accounts.escrow)?;
        escrow.set_inner(
            self.instruction_data.seed,
            self.accounts.maker.address().clone(),
            self.accounts.mint_a.address().clone(),
            self.accounts.mint_b.address().clone(),
            self.instruction_data.receive,
            escorw_bump_binding,
        );

        // --------------- 创建Vault ATA 账户 ------------------
        Create {
            funding_account: self.accounts.maker,
            account: self.accounts.vault,
            wallet: self.accounts.escrow,
            mint: self.accounts.mint_a,
            system_program: self.accounts.system_program,
            token_program: self.accounts.token_program,
        }
        .invoke()?;

        // --------------- 存储 Token A 代币 ------------------
        Transfer {
            from: self.accounts.maker_ata_a,
            to: self.accounts.vault,
            authority: self.accounts.maker,
            amount: self.instruction_data.amount,
        }
        .invoke()?;

        Ok(())
    }
}
