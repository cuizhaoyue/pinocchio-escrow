use crate::{
    errors::EscrowError,
    instructions::{
        AssociatedTokenAccount, MintInterface, ProgramAccount, SignerAccount, SystemProgram,
        TokenAccount, TokenProgram,
    },
    Escrow,
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address, ProgramResult,
};
use pinocchio_token::instructions::{CloseAccount, Transfer};

// ============================
// 新手说明（refund）
// refund = 取消挂单:
// 1) 把 vault 里 token A 全部退还 maker
// 2) 关闭 vault
// 3) 关闭 escrow
//
// 关键限制:
// - 只有 maker 可以发起（maker 必须签名，且与 escrow 内记录一致）
// ============================

// refund 指令账户。
pub struct RefundAccounts<'a> {
    // 挂单方（仅 maker 可退款）。
    pub maker: &'a AccountView,
    // escrow PDA。
    pub escrow: &'a AccountView,
    pub mint_a: &'a AccountView,
    // escrow 的 token A vault ATA。
    pub vault: &'a AccountView,
    // maker 的 token A ATA（退款目标）。
    pub maker_ata_a: &'a AccountView,
    pub system_program: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for RefundAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        // 基于固定前缀顺序取账户，兼容 challenge 用例。
        let [maker, escrow, mint_a, maker_ata_a, vault, system_program, token_program, ..] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // 基础账户校验。
        SignerAccount::check(maker)?;
        ProgramAccount::check(escrow)?;
        MintInterface::check(mint_a)?;
        SystemProgram::check(system_program)?;
        TokenProgram::check(token_program)?;

        // 推导 maker_ata_a / vault 的预期 ATA 地址。
        let (expected_maker_ata_key, _) = Address::find_program_address(
            &[
                maker.address().as_ref(),
                token_program.address().as_ref(),
                mint_a.address().as_ref(),
            ],
            &pinocchio_associated_token_account::ID,
        );
        let (expected_vault_key, _) = Address::find_program_address(
            &[
                escrow.address().as_ref(),
                token_program.address().as_ref(),
                mint_a.address().as_ref(),
            ],
            &pinocchio_associated_token_account::ID,
        );

        // 兼容外部传参把 vault 与 maker_ata_a 位置互换的情况。
        // 也就是说：即便第 4/5 个账户顺序传反，只要地址正确也能识别。
        let (maker_ata_a, vault) = if maker_ata_a.address() == &expected_maker_ata_key
            && vault.address() == &expected_vault_key
        {
            (maker_ata_a, vault)
        } else if maker_ata_a.address() == &expected_vault_key
            && vault.address() == &expected_maker_ata_key
        {
            (vault, maker_ata_a)
        } else {
            return Err(EscrowError::InvalidAddress.into());
        };

        // maker_ata_a 可能未初始化，先幂等创建再做 ATA 校验。
        // 这样可以避免“账户 owner/data 无效”这类典型新手报错。
        AssociatedTokenAccount::init_if_needed(
            maker_ata_a,
            mint_a,
            maker,
            maker,
            system_program,
            token_program,
        )?;
        AssociatedTokenAccount::check(maker_ata_a, maker, mint_a, token_program)?;
        AssociatedTokenAccount::check(vault, escrow, mint_a, token_program)?;

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
    pub escrow_seed: u64,
    pub escrow_bump: [u8; 1],
}

impl<'a> TryFrom<&'a [AccountView]> for RefundContext<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let refund_accounts = RefundAccounts::try_from(accounts)?;
        let escrow = Escrow::load(refund_accounts.escrow)?;

        if refund_accounts.maker.address() != escrow.maker() {
            return Err(EscrowError::InvalidMaker.into());
        }
        if refund_accounts.mint_a.address() != escrow.mint_a() {
            return Err(ProgramError::InvalidAccountData);
        }

        let seed_bytes = escrow.seed().to_le_bytes();
        let escrow_key = Address::create_program_address(
            &[
                b"escrow",
                refund_accounts.maker.address().as_ref(),
                &seed_bytes,
                &escrow.bump(),
            ],
            &crate::ID,
        )?;
        if refund_accounts.escrow.address() != &escrow_key {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            accounts: refund_accounts,
            escrow_seed: escrow.seed(),
            escrow_bump: escrow.bump(),
        })
    }
}

impl<'a> RefundContext<'a> {
    pub const DISCRIMINATOR: u8 = 2;

    pub fn process(&self) -> ProgramResult {
        // escrow 状态在 TryFrom 阶段已经解析并校验过，这里直接复用。
        let seed_bytes = self.escrow_seed.to_le_bytes();

        let signer_seeds = [
            Seed::from(b"escrow"),
            Seed::from(self.accounts.maker.address().as_ref()),
            Seed::from(&seed_bytes),
            Seed::from(&self.escrow_bump),
        ];
        // escrow PDA 作为 authority，需要 invoke_signed。
        let signer = [Signer::from(&signer_seeds)];

        // 1) vault -> maker_ata_a: 退回全部 token A。
        let amount = TokenAccount::amount(self.accounts.vault)?;
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.maker_ata_a,
            authority: self.accounts.escrow,
            amount,
        }
        .invoke_signed(&signer)?;

        // 2) 关闭 vault，回收 rent 给 maker。
        CloseAccount {
            account: self.accounts.vault,
            destination: self.accounts.maker,
            authority: self.accounts.escrow,
        }
        .invoke_signed(&signer)?;

        // 3) 关闭 escrow 主账户，回收 rent 给 maker。
        ProgramAccount::close(self.accounts.escrow, self.accounts.maker)
    }
}
