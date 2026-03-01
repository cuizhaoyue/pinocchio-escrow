use {
    mollusk_svm::{result::ProgramResult, Mollusk},
    solana_account::Account,
    solana_instruction::Instruction,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    std::{format, path::Path, vec},
};

const PROGRAM_NAME: &str = "pinocchio_escrow";

/// 将程序里的 `Address` 转为测试中常用的 `Pubkey`。
fn program_id() -> Pubkey {
    Pubkey::new_from_array(*crate::ID.as_array())
}

#[test]
fn test_invalid_discriminator_returns_invalid_instruction_data() {
    // 1) 指定 SBF 输出目录，让 Mollusk 能找到已编译的程序 so。
    let out_dir = format!("{}/target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("SBF_OUT_DIR", &out_dir);

    // 2) 如果 so 不存在，直接给出清晰提示，避免报错信息不直观。
    let so_path = format!("{out_dir}/{PROGRAM_NAME}.so");
    assert!(
        Path::new(&so_path).exists(),
        "未找到程序文件 `{so_path}`，请先执行 `cargo build-sbf`。"
    );

    // 3) 加载待测程序。
    let mollusk = Mollusk::new(&program_id(), PROGRAM_NAME);

    // 4) 构造一个非法 discriminator（当前程序仅支持 0/1/2）。
    let ix = Instruction::new_with_bytes(program_id(), &[9u8], vec![]);
    let accounts: [(Pubkey, Account); 0] = [];

    // 5) 断言程序返回 InvalidInstructionData。
    let result = mollusk.process_instruction(&ix, &accounts);
    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidInstructionData)
    );
}
