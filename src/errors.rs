use core::fmt;
use pinocchio::error::ProgramError;

// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub enum EscrowError {
//     NotRentExempt = 0,
//     NotSigner = 1,
//     InvalidOwner = 2,
//     InvalidAccountData = 3,
//     InvalidAddress = 4,
// }

// impl From<EscrowError> for ProgramError {
//     fn from(error: EscrowError) -> Self {
//         ProgramError::Custom(error as u32)
//     }
// }

// 业务自定义错误码，最终映射到 ProgramError::Custom(u32)。
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EscrowError {
    // 传入账户地址与预期 PDA/ATA 地址不一致。
    InvalidAddress = 0,
    // 传入 maker 与 escrow 记录的 maker 不一致。
    InvalidMaker = 1,
    // system_program 非系统程序账户。
    InvalidSystemProgram = 2,
    // token_program 非 SPL Token 程序账户。
    InvalidTokenProgram = 3,
    InvalidOwner = 4,
}

impl From<EscrowError> for ProgramError {
    fn from(value: EscrowError) -> Self {
        ProgramError::Custom(value as u32)
    }
}

// 错误信息，用于本地调试
impl fmt::Display for EscrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // EscrowError::NotRentExempt => write!(f, "Lamport balance below rent-exempt threshold"),
            // EscrowError::NotSigner => write!(f, "没有签名"),
            // EscrowError::InvalidOwner => write!(f, "非法的所有者"),
            // EscrowError::InvalidAccountData => write!(f, "非法的账户数据"),
            // EscrowError::InvalidAddress => write!(f, "非法的地址"),
            EscrowError::InvalidAddress => write!(f, "非法的地址"),
            EscrowError::InvalidMaker => write!(f, "maker 不一致"),
            EscrowError::InvalidSystemProgram => write!(f, "system_program 非系统程序账户"),
            EscrowError::InvalidTokenProgram => write!(f, "token_program 非 SPL Token 账户"),
            EscrowError::InvalidOwner => write!(f, "非法的所有者"),
        }
    }
}
