use crate::{
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
// 新手说明（take）
// take = 吃单:
// 1) taker 从 vault 拿到 token A
// 2) taker 支付 token B 给 maker
// 3) 关闭 vault 和 escrow，回收 rent
//
// 这里也分两段:
// - TryFrom: 账户准备（含 init_if_needed）
// - process: 实际转账与关户
// ============================

// take 指令账户（按指南顺序）。
pub struct TakeAccounts<'a> {
    // 吃单方（签名者）。
    pub taker: &'a AccountView,
    // 挂单方。
    pub maker: &'a AccountView,
    // escrow PDA。
    pub escrow: &'a AccountView,
    pub mint_a: &'a AccountView,
    pub mint_b: &'a AccountView,
    // escrow 的 token A vault ATA。
    pub vault: &'a AccountView,
    // taker 的 token A ATA（接收 maker 锁仓的 token A）。
    pub taker_ata_a: &'a AccountView,
    // taker 的 token B ATA（支付来源）。
    pub taker_ata_b: &'a AccountView,
    // maker 的 token B ATA（收款目标）。
    pub maker_ata_b: &'a AccountView,
    pub system_program: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for TakeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        // 固定账户顺序解析，兼容 challenge 测试。
        let [taker, maker, escrow, mint_a, mint_b, vault, taker_ata_a, taker_ata_b, maker_ata_b, system_program, token_program, _] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // 基础账户校验。
        SignerAccount::check(taker)?;
        MintInterface::check(mint_a)?;
        MintInterface::check(mint_b)?;
        AssociatedTokenAccount::check(taker_ata_a, taker, mint_a, token_program)?;
        AssociatedTokenAccount::check(taker_ata_b, taker, mint_b, token_program)?;
        AssociatedTokenAccount::check(maker_ata_b, maker, mint_b, token_program)?;
        AssociatedTokenAccount::check(vault, escrow, mint_a, token_program)?;
        SystemProgram::check(system_program)?;
        TokenProgram::check(token_program)?;

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
    pub escrow_seed: u64,
    pub escrow_receive: u64,
    pub escrow_bump: [u8; 1],
}

impl<'a> TryFrom<&'a [AccountView]> for TakeContext<'a> {
    type Error = ProgramError;
    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        // 完整校验 + 读取 escrow 业务字段一致性。
        // 这里会比基础账户校验更严格，确保拿到的是“这张订单对应的 mint/maker”。
        let accounts = TakeAccounts::try_from(accounts)?;
        ProgramAccount::check(accounts.escrow)?;
        let escrow_state = Escrow::load(accounts.escrow)?;

        if accounts.maker.address() != escrow_state.maker() {
            return Err(ProgramError::InvalidAccountData);
        }
        if accounts.mint_a.address() != escrow_state.mint_a() {
            return Err(ProgramError::InvalidAccountData);
        }
        if accounts.mint_b.address() != escrow_state.mint_b() {
            return Err(ProgramError::InvalidAccountData);
        }

        // 允许 taker 或 maker_ata_b 尚未存在，先幂等初始化。
        // 常见场景: taker 第一次接触该 mint，还没有对应 ATA。
        AssociatedTokenAccount::init_if_needed(
            accounts.taker_ata_a,
            accounts.mint_a,
            accounts.taker,
            accounts.taker,
            accounts.system_program,
            accounts.token_program,
        )?;

        AssociatedTokenAccount::init_if_needed(
            accounts.maker_ata_b,
            accounts.mint_b,
            accounts.taker,
            accounts.maker,
            accounts.system_program,
            accounts.token_program,
        )?;

        Ok(Self {
            accounts,
            escrow_seed: escrow_state.seed(),
            escrow_receive: escrow_state.receive(),
            escrow_bump: escrow_state.bump(),
        })
    }
}

impl<'a> TakeContext<'a> {
    pub const DISCRIMINATOR: u8 = 1;

    pub fn process(&self) -> ProgramResult {
        // escrow 状态在 TryFrom 阶段已经解析并校验过，这里直接复用。
        let seed_bytes = self.escrow_seed.to_le_bytes();
        let escrow_key = Address::create_program_address(
            &[
                b"escrow",
                self.accounts.maker.address().as_ref(),
                &seed_bytes,
                &self.escrow_bump,
            ],
            &crate::ID,
        )?;
        if self.accounts.escrow.address() != &escrow_key {
            return Err(ProgramError::InvalidAccountData);
        }

        let signer_seeds = [
            Seed::from(b"escrow"),
            Seed::from(self.accounts.maker.address().as_ref()),
            Seed::from(&seed_bytes),
            Seed::from(&self.escrow_bump),
        ];
        // invoke_signed 需要 signer 列表；这里 signer 代表 escrow PDA。
        let signer = [Signer::from(&signer_seeds)];

        // 1) vault -> taker_ata_a: 转出 token A。
        let amount = TokenAccount::amount(self.accounts.vault)?;
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.taker_ata_a,
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

        // 3) taker -> maker: 支付 token B。
        Transfer {
            from: self.accounts.taker_ata_b,
            to: self.accounts.maker_ata_b,
            authority: self.accounts.taker,
            amount: self.escrow_receive,
        }
        .invoke()?;

        // 4) 关闭 escrow 主账户，回收 rent 给 taker（与指南一致）。
        ProgramAccount::close(self.accounts.escrow, self.accounts.taker)
    }
}
