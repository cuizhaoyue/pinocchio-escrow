use crate::{errors::EscrowError, Escrow};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_associated_token_account::instructions::{Create, CreateIdempotent};
use pinocchio_system::instructions::CreateAccount;
use solana_program_log::log;

// ============================
// 新手说明（helper 模块）
// 这个文件放的是“通用积木”，避免 make/take/refund 重复写样板代码。
//
// 常见概念:
// - PDA: Program Derived Address，程序推导地址，通常作为“程序控制的钱包/状态账户”
// - ATA: Associated Token Account，某 (owner, mint, token_program) 的标准代币账户
// - AccountView: pinocchio 提供的账户只读/可变视图
// ============================

// 通用签名者账户校验。
pub struct SignerAccount;
impl SignerAccount {
    #[inline(always)]
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}

// System Program 账户校验。
pub struct SystemProgram;
impl SystemProgram {
    #[inline(always)]
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if account.address() != &pinocchio_system::ID || !account.executable() {
            return Err(EscrowError::InvalidSystemProgram.into());
        }
        Ok(())
    }
}

// SPL Token Program 账户校验。
pub struct TokenProgram;
impl TokenProgram {
    #[inline(always)]
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if account.address() != &pinocchio_token::ID || !account.executable() {
            return Err(EscrowError::InvalidTokenProgram.into());
        }
        Ok(())
    }
}

// Mint 账户基础校验（owner + data 长度）。
pub struct MintInterface;
impl MintInterface {
    #[inline(always)]
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&pinocchio_token::ID) {
            println!("{}", EscrowError::InvalidOwner);
            return Err(EscrowError::InvalidOwner.into());
        }
        if account.data_len() != pinocchio_token::state::Mint::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

// Token Account 基础校验与读取辅助。
pub struct TokenAccount;
impl TokenAccount {
    #[inline(always)]
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&pinocchio_token::ID) {
            log(EscrowError::InvalidOwner.to_string().as_str());
            return Err(EscrowError::InvalidOwner.into());
        }
        if account.data_len() != pinocchio_token::state::TokenAccount::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    pub fn amount(account: &AccountView) -> Result<u64, ProgramError> {
        // 由 pinocchio-token 负责解析 token account 布局并读取 amount。
        Ok(pinocchio_token::state::TokenAccount::from_account_view(account)?.amount())
    }
}

// ATA 派生与创建辅助。
pub struct AssociatedTokenAccount;
impl AssociatedTokenAccount {
    #[inline(always)]
    pub fn check(
        account: &AccountView,
        authority: &AccountView,
        mint: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError> {
        TokenAccount::check(account)?;

        // ATA 地址推导规则: [authority, token_program, mint] + ATA program id。
        // 新手可记成: “同一个 authority + mint，只会有一个标准 ATA 地址”。
        let (derived_ata, _) = Address::find_program_address(
            &[
                authority.address().as_ref(),
                token_program.address().as_ref(),
                mint.address().as_ref(),
            ],
            &pinocchio_associated_token_account::ID,
        );

        if account.address() != &derived_ata {
            return Err(EscrowError::InvalidAddress.into());
        }

        Ok(())
    }

    #[inline(always)]
    pub fn init(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        authority: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult {
        // 非幂等创建：账户已存在会失败。
        // 适用于“我们确定这个账户现在应该还不存在”的场景。
        Create {
            funding_account: payer,
            account,
            wallet: authority,
            mint,
            system_program,
            token_program,
        }
        .invoke()
    }

    #[inline(always)]
    pub fn init_if_needed(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        authority: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult {
        // 幂等创建：账户存在时不会报错，便于 take/refund 自动补齐目标 ATA。
        // 对新手最友好：调用前不用先判断账户是否存在。
        CreateIdempotent {
            funding_account: payer,
            account,
            wallet: authority,
            mint,
            system_program,
            token_program,
        }
        .invoke()
    }
}

// 程序自有 Escrow 账户的校验、创建与关闭逻辑。
pub struct ProgramAccount;
impl ProgramAccount {
    #[inline(always)]
    pub fn check(account: &AccountView) -> Result<(), ProgramError> {
        if !account.owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        if account.data_len() != Escrow::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    pub fn init<'a>(
        account: &AccountView,
        payer: &AccountView,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult {
        // 以 rent exemption 的 lamports 创建 PDA 账户。
        // seeds + invoke_signed => 让运行时认可“这是 PDA 的合法签名”。
        let lamports = Rent::get()?.try_minimum_balance(space)?;
        let signer = [Signer::from(seeds)];

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&signer)
    }

    #[inline(always)]
    pub fn close(account: &AccountView, destination: &AccountView) -> ProgramResult {
        // 先标记数据首字节，避免后续误解读为有效状态。
        {
            let mut data = account.try_borrow_mut()?;
            if !data.is_empty() {
                data[0] = 0xff;
            }
        }

        /* 经过测试，删除上面的代码和下面的resize函数也可以正常运行，ChatGPT分析后也是建议删除 */

        // 回收 lamports 到 destination，并关闭账户。
        // 这里手动转 lamports 的原因是 no_std + 轻量实现，不依赖 Anchor 宏。
        let destination_lamports = destination.lamports();
        let account_lamports = account.lamports();
        let new_destination_lamports = destination_lamports
            .checked_add(account_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        destination.set_lamports(new_destination_lamports);
        account.set_lamports(0);
        account.resize(1)?;
        account.close()
    }
}
