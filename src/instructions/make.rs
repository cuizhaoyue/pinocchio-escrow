use crate::{
    AssociatedTokenAccount, Escrow, MintInterface, ProgramAccount, SignerAccount, SystemProgram,
    TokenProgram,
};
use core::mem::size_of;
use pinocchio::{cpi::Seed, error::ProgramError, AccountView, Address, ProgramResult};
use pinocchio_token::instructions::Transfer;

// ============================
// 新手说明（make）
// make = 挂单:
// 1) 创建 escrow PDA 状态账户（记录交易参数）
// 2) 创建 vault ATA（owner=escrow）
// 3) maker 把 token A 转入 vault
//
// 代码分两段:
// - TryFrom: 解析并校验账户、初始化需要的新账户
// - process: 真正写状态 + 转账
// ============================

// make 指令账户（按指南顺序）。
pub struct MakeAccounts<'a> {
    // 挂单者（签名者，payer）。
    pub maker: &'a AccountView,
    // escrow PDA（将被创建）。
    pub escrow: &'a AccountView,
    // 存入资产 mint。
    pub mint_a: &'a AccountView,
    // 期望收到资产 mint。
    pub mint_b: &'a AccountView,
    // maker 的 token A ATA（资金来源）。
    pub maker_ata_a: &'a AccountView,
    // vault ATA（authority=escrow，mint=mint_a）。
    pub vault: &'a AccountView,
    pub system_program: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for MakeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        // 固定账户顺序解析，兼容 Blueshift challenge 测试。
        let [maker, escrow, mint_a, mint_b, maker_ata_a, vault, system_program, token_program, _] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // 基础账户校验。
        SignerAccount::check(maker)?;
        MintInterface::check(mint_a)?;
        MintInterface::check(mint_b)?;
        AssociatedTokenAccount::check(maker_ata_a, maker, mint_a, token_program)?;
        SystemProgram::check(system_program)?;
        TokenProgram::check(token_program)?;

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

#[repr(C)]
pub struct MakeInstructionData {
    // PDA 种子参数。
    pub seed: u64,
    // maker 希望拿到的 token B 数量。
    pub receive: u64,
    // maker 实际锁入 vault 的 token A 数量。
    pub amount: u64,
}

impl TryFrom<&[u8]> for MakeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // make 指令负载必须是 3 个 u64。
        // 顺序约定: seed | receive | amount（小端序）。
        if data.len() != size_of::<Self>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed = u64::from_le_bytes(
            data.get(0..8)
                .ok_or(ProgramError::InvalidInstructionData)?
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );
        let receive = u64::from_le_bytes(
            data.get(8..16)
                .ok_or(ProgramError::InvalidInstructionData)?
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );
        let amount = u64::from_le_bytes(
            data.get(16..24)
                .ok_or(ProgramError::InvalidInstructionData)?
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );

        if receive == 0 || amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

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
    pub bump: [u8; 1],
}

impl<'a> TryFrom<(&[u8], &'a [AccountView])> for MakeContext<'a> {
    type Error = ProgramError;

    fn try_from(value: (&[u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let (data, account_infos) = value;
        let instruction_data = MakeInstructionData::try_from(data)?;
        let accounts = MakeAccounts::try_from(account_infos)?;

        // 推导 escrow PDA 并校验传入地址。
        // 规则: ["escrow", maker, seed]
        let seed_bytes = instruction_data.seed.to_le_bytes();
        let (escrow_key, bump) = Address::find_program_address(
            &[b"escrow", accounts.maker.address().as_ref(), &seed_bytes],
            &crate::ID,
        );
        if accounts.escrow.address() != &escrow_key {
            return Err(ProgramError::InvalidAccountData);
        }

        let bump_bytes = [bump];
        let escrow_seeds = [
            Seed::from(b"escrow"),
            Seed::from(accounts.maker.address().as_ref()),
            Seed::from(&seed_bytes),
            Seed::from(&bump_bytes),
        ];

        // 初始化 escrow 账户与 vault ATA。
        // escrow 是 PDA 账户（owner=本程序），vault 是 escrow 对应的 ATA。
        ProgramAccount::init(accounts.escrow, accounts.maker, &escrow_seeds, Escrow::LEN)?;
        AssociatedTokenAccount::init(
            accounts.vault,
            accounts.mint_a,
            accounts.maker,
            accounts.escrow,
            accounts.system_program,
            accounts.token_program,
        )?;

        Ok(Self {
            accounts,
            instruction_data,
            bump: [bump],
        })
    }
}

impl<'a> MakeContext<'a> {
    pub const DISCRIMINATOR: u8 = 0;

    pub fn process(&self) -> ProgramResult {
        // 1) 写入 escrow 状态。
        // 注意: 这里会直接修改 escrow 账户数据，所以使用 try_borrow_mut。
        // let mut escrow_data = self.accounts.escrow.try_borrow_mut()?;
        let mut escrow = Escrow::load_mut(self.accounts.escrow)?;
        escrow.set_inner(
            self.instruction_data.seed,
            self.accounts.maker.address().clone(),
            self.accounts.mint_a.address().clone(),
            self.accounts.mint_b.address().clone(),
            self.instruction_data.receive,
            self.bump,
        );

        // 2) maker -> vault 转入 token A。
        // authority = maker，表示需要 maker 签名授权转账。
        Transfer {
            from: self.accounts.maker_ata_a,
            to: self.accounts.vault,
            authority: self.accounts.maker,
            amount: self.instruction_data.amount,
        }
        .invoke()
    }
}
