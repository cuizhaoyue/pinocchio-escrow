use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
use pinocchio_associated_token_account::instructions::CreateIdempotent;

/// 校验账户是否是签名者
pub fn check_signer(account: &AccountView) -> ProgramResult {
    if !account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    Ok(())
}

/// 校验程序账户: 地址匹配且可执行
#[inline(always)]
pub fn check_program(
    program_account: &AccountView,
    expected_program_id: &Address,
) -> ProgramResult {
    if program_account.address() != expected_program_id || !program_account.executable() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// 校验一组 u64 类型数值必须都大于0
#[inline(always)]
pub fn check_non_zero(values: &[u64]) -> ProgramResult {
    if values.iter().any(|v| *v == 0) {
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

pub fn verify_mint_account(account: &AccountView) -> ProgramResult {
    if !account.owned_by(&pinocchio_token::ID) {
        return Err(ProgramError::InvalidAccountData);
    }
    if account.data_len() != pinocchio_token::state::Mint::LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

pub fn close(account: &AccountView, destination: &AccountView) -> ProgramResult {
    let account_balance = account.lamports();
    let destination_balance = destination
        .lamports()
        .checked_add(account_balance)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    destination.set_lamports(destination_balance);
    account.set_lamports(0);
    account.close()
}

pub fn init_ata_if_needed(
    account: &AccountView,
    mint: &AccountView,
    payer: &AccountView,
    authority: &AccountView,
    system_program: &AccountView,
    token_program: &AccountView,
) -> ProgramResult {
    CreateIdempotent {
        funding_account: payer,
        account,
        mint,
        wallet: authority,
        system_program,
        token_program,
    }
    .invoke()?;

    Ok(())
}
