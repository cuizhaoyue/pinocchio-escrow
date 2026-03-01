use {
    mollusk_svm::{result::ProgramResult, Mollusk},
    mollusk_svm_programs_token::{associated_token, token},
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramError,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
    spl_token_interface::state::{Account as SplTokenAccount, AccountState, Mint},
    std::{collections::HashMap, format, path::Path, vec},
};

const PROGRAM_NAME: &str = "pinocchio_escrow";

/// 将当前程序的 `Address` 转成测试里通用的 `Pubkey`。
fn program_id() -> Pubkey {
    Pubkey::new_from_array(*crate::ID.as_array())
}

/// 统一创建 Mollusk 测试环境，并加载 escrow/token/ata 三个程序。
fn setup_mollusk() -> Mollusk {
    let out_dir = format!("{}/target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("SBF_OUT_DIR", &out_dir);

    // 这里依赖已编译的 SBF so 文件；缺失时给出明确报错提示。
    let so_file = format!("{out_dir}/{PROGRAM_NAME}.so");
    assert!(
        Path::new(&so_file).exists(),
        "未找到程序文件 `{so_file}`，请先执行 `cargo build-sbf`。"
    );

    let mut mollusk = Mollusk::new(&program_id(), PROGRAM_NAME);
    token::add_program(&mut mollusk);
    associated_token::add_program(&mut mollusk);
    mollusk
}

/// 生成一个普通系统账户（用于 maker / taker）。
fn system_account(lamports: u64) -> Account {
    Account::new(lamports, 0, &solana_sdk_ids::system_program::id())
}

/// 生成一个已初始化的 SPL Mint 账户。
fn mint_account(decimals: u8) -> Account {
    token::create_account_for_mint(Mint {
        mint_authority: COption::None,
        supply: 1_000_000_000_000,
        decimals,
        is_initialized: true,
        freeze_authority: COption::None,
    })
}

/// 计算指定 `(owner, mint)` 对应的 ATA 地址（使用标准 Token 程序）。
fn ata_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, &token::ID)
}

/// 构造一个已初始化的 TokenAccount 数据账户（地址需由调用方保证是正确 ATA）。
fn token_account(owner: &Pubkey, mint: &Pubkey, amount: u64) -> Account {
    token::create_account_for_token_account(SplTokenAccount {
        mint: *mint,
        owner: *owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    })
}

/// 从账户数据里解包并读取 SPL Token 余额。
fn token_amount(account: &Account) -> u64 {
    SplTokenAccount::unpack(account.data.as_slice())
        .expect("token 账户数据反序列化失败")
        .amount
}

/// 解析 escrow 账户中的关键字段（seed / receive / bump）。
fn escrow_fields(account: &Account) -> (u64, u64, [u8; 1]) {
    let data = account.data.as_slice();
    assert_eq!(
        data.len(),
        crate::Escrow::LEN,
        "escrow 账户数据长度不正确，期望 {}，实际 {}",
        crate::Escrow::LEN,
        data.len()
    );
    let seed = u64::from_le_bytes(data[0..8].try_into().expect("seed 解析失败"));
    let receive = u64::from_le_bytes(data[104..112].try_into().expect("receive 解析失败"));
    let bump = [data[112]];
    (seed, receive, bump)
}

/// make 指令：`[discriminator=0 | seed | receive | amount]`
fn make_ix(
    maker: Pubkey,
    escrow: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    maker_ata_a: Pubkey,
    vault: Pubkey,
    seed: u64,
    receive: u64,
    amount: u64,
) -> Instruction {
    let mut data = vec![0u8];
    data.extend_from_slice(&seed.to_le_bytes());
    data.extend_from_slice(&receive.to_le_bytes());
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction::new_with_bytes(
        program_id(),
        &data,
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(mint_a, false),
            AccountMeta::new_readonly(mint_b, false),
            AccountMeta::new(maker_ata_a, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(associated_token::ID, false),
        ],
    )
}

/// take 指令：`[discriminator=1]`
fn take_ix(
    taker: Pubkey,
    maker: Pubkey,
    escrow: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    vault: Pubkey,
    taker_ata_a: Pubkey,
    taker_ata_b: Pubkey,
    maker_ata_b: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id(),
        &[1u8],
        vec![
            AccountMeta::new(taker, true),
            AccountMeta::new(maker, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(mint_a, false),
            AccountMeta::new_readonly(mint_b, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(taker_ata_a, false),
            AccountMeta::new(taker_ata_b, false),
            AccountMeta::new(maker_ata_b, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(associated_token::ID, false),
        ],
    )
}

/// refund 指令：`[discriminator=2]`
fn refund_ix(
    maker: Pubkey,
    escrow: Pubkey,
    mint_a: Pubkey,
    maker_ata_a: Pubkey,
    vault: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id(),
        &[2u8],
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(mint_a, false),
            AccountMeta::new(maker_ata_a, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(associated_token::ID, false),
        ],
    )
}

#[test]
fn test_make_success_with_mollusk_unit() {
    let mollusk = setup_mollusk();

    // 准备基础地址。
    let maker = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let maker_ata_a = ata_address(&maker, &mint_a);

    // escrow PDA 与 vault ATA 必须和程序内推导规则完全一致。
    let seed = 11u64;
    let receive = 222u64;
    let amount = 90u64;
    let seed_bytes = seed.to_le_bytes();
    let (escrow_pda, bump) =
        Pubkey::find_program_address(&[b"escrow", maker.as_ref(), &seed_bytes], &program_id());
    let vault = ata_address(&escrow_pda, &mint_a);

    // 初始账户：maker 持有 token A，其余待创建账户先不放入 store。
    let mut store = HashMap::new();
    store.insert(maker, system_account(5_000_000_000));
    store.insert(mint_a, mint_account(6));
    store.insert(mint_b, mint_account(6));
    store.insert(maker_ata_a, token_account(&maker, &mint_a, 500));
    let context = mollusk.with_context(store);

    // 执行 make。
    let result = context.process_instruction(&make_ix(
        maker,
        escrow_pda,
        mint_a,
        mint_b,
        maker_ata_a,
        vault,
        seed,
        receive,
        amount,
    ));
    assert_eq!(result.program_result, ProgramResult::Success);

    // 校验状态变更：maker_ata_a 扣款、vault 收款、escrow 数据正确写入。
    let store = context.account_store.borrow();
    assert_eq!(
        token_amount(store.get(&maker_ata_a).expect("maker_ata_a 不存在")),
        500 - amount
    );
    assert_eq!(
        token_amount(store.get(&vault).expect("vault 不存在")),
        amount
    );

    let escrow_account = store.get(&escrow_pda).expect("escrow 账户不存在");
    let (saved_seed, saved_receive, saved_bump) = escrow_fields(escrow_account);
    assert_eq!(saved_seed, seed);
    assert_eq!(saved_receive, receive);
    assert_eq!(saved_bump, [bump]);
}

#[test]
fn test_take_success_with_mollusk_unit() {
    let mollusk = setup_mollusk();

    // 参与方与资产地址。
    let maker = Pubkey::new_unique();
    let taker = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let maker_ata_a = ata_address(&maker, &mint_a);
    let taker_ata_b = ata_address(&taker, &mint_b);

    // 订单参数。
    let seed = 2026u64;
    let amount_a = 120u64;
    let receive_b = 350u64;
    let seed_bytes = seed.to_le_bytes();
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[b"escrow", maker.as_ref(), &seed_bytes], &program_id());
    let vault = ata_address(&escrow_pda, &mint_a);
    let taker_ata_a = ata_address(&taker, &mint_a);
    let maker_ata_b = ata_address(&maker, &mint_b);

    // 初始状态：maker 有 token A，taker 有足够 token B。
    let mut store = HashMap::new();
    store.insert(maker, system_account(5_000_000_000));
    store.insert(taker, system_account(5_000_000_000));
    store.insert(mint_a, mint_account(6));
    store.insert(mint_b, mint_account(6));
    store.insert(maker_ata_a, token_account(&maker, &mint_a, 700));
    store.insert(taker_ata_b, token_account(&taker, &mint_b, 1_000));
    let context = mollusk.with_context(store);

    // 先挂单，再吃单。
    let make_result = context.process_instruction(&make_ix(
        maker,
        escrow_pda,
        mint_a,
        mint_b,
        maker_ata_a,
        vault,
        seed,
        receive_b,
        amount_a,
    ));
    assert_eq!(make_result.program_result, ProgramResult::Success);

    let take_result = context.process_instruction(&take_ix(
        taker,
        maker,
        escrow_pda,
        mint_a,
        mint_b,
        vault,
        taker_ata_a,
        taker_ata_b,
        maker_ata_b,
    ));
    assert_eq!(take_result.program_result, ProgramResult::Success);

    // 校验资产交换结果。
    let store = context.account_store.borrow();
    assert_eq!(
        token_amount(store.get(&maker_ata_a).expect("maker_ata_a 不存在")),
        700 - amount_a
    );
    assert_eq!(
        token_amount(store.get(&taker_ata_a).expect("taker_ata_a 不存在")),
        amount_a
    );
    assert_eq!(
        token_amount(store.get(&taker_ata_b).expect("taker_ata_b 不存在")),
        1_000 - receive_b
    );
    assert_eq!(
        token_amount(store.get(&maker_ata_b).expect("maker_ata_b 不存在")),
        receive_b
    );
}

#[test]
fn test_refund_success_with_mollusk_unit() {
    let mollusk = setup_mollusk();

    // maker 挂单后主动撤单，token A 应原路退回。
    let maker = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let maker_ata_a = ata_address(&maker, &mint_a);

    let seed = 88u64;
    let amount_a = 180u64;
    let receive_b = 520u64;
    let seed_bytes = seed.to_le_bytes();
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[b"escrow", maker.as_ref(), &seed_bytes], &program_id());
    let vault = ata_address(&escrow_pda, &mint_a);

    let mut store = HashMap::new();
    store.insert(maker, system_account(5_000_000_000));
    store.insert(mint_a, mint_account(6));
    store.insert(mint_b, mint_account(6));
    store.insert(maker_ata_a, token_account(&maker, &mint_a, 700));
    let context = mollusk.with_context(store);

    let make_result = context.process_instruction(&make_ix(
        maker,
        escrow_pda,
        mint_a,
        mint_b,
        maker_ata_a,
        vault,
        seed,
        receive_b,
        amount_a,
    ));
    assert_eq!(make_result.program_result, ProgramResult::Success);

    let refund_result =
        context.process_instruction(&refund_ix(maker, escrow_pda, mint_a, maker_ata_a, vault));
    assert_eq!(refund_result.program_result, ProgramResult::Success);

    // 撤单后：maker 的 token A 恢复到初始值，escrow/vault 关闭。
    let store = context.account_store.borrow();
    assert_eq!(
        token_amount(store.get(&maker_ata_a).expect("maker_ata_a 不存在")),
        700
    );

    let escrow_after_refund = store.get(&escrow_pda).expect("escrow 不存在");
    assert_eq!(escrow_after_refund.lamports, 0);
    assert!(escrow_after_refund.data.len() <= 1);

    let vault_after_refund = store.get(&vault).expect("vault 不存在");
    assert_eq!(vault_after_refund.lamports, 0);
}

#[test]
fn test_refund_reject_non_maker_with_mollusk_unit() {
    let mollusk = setup_mollusk();

    // 先由 maker 创建订单，再让 attacker 假冒 maker 调用 refund，预期失败。
    let maker = Pubkey::new_unique();
    let attacker = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let maker_ata_a = ata_address(&maker, &mint_a);
    let attacker_ata_a = ata_address(&attacker, &mint_a);

    let seed = 66u64;
    let amount_a = 100u64;
    let receive_b = 300u64;
    let seed_bytes = seed.to_le_bytes();
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[b"escrow", maker.as_ref(), &seed_bytes], &program_id());
    let vault = ata_address(&escrow_pda, &mint_a);

    let mut store = HashMap::new();
    store.insert(maker, system_account(5_000_000_000));
    store.insert(attacker, system_account(5_000_000_000));
    store.insert(mint_a, mint_account(6));
    store.insert(mint_b, mint_account(6));
    store.insert(maker_ata_a, token_account(&maker, &mint_a, 500));
    let context = mollusk.with_context(store);

    let make_result = context.process_instruction(&make_ix(
        maker,
        escrow_pda,
        mint_a,
        mint_b,
        maker_ata_a,
        vault,
        seed,
        receive_b,
        amount_a,
    ));
    assert_eq!(make_result.program_result, ProgramResult::Success);

    // attacker 作为第一个账户传入，会触发 InvalidMaker（错误码 1）。
    let bad_refund = context.process_instruction(&refund_ix(
        attacker,
        escrow_pda,
        mint_a,
        attacker_ata_a,
        vault,
    ));
    assert_eq!(
        bad_refund.program_result,
        ProgramResult::Failure(ProgramError::Custom(1))
    );

    // 指令失败后，Context 不会持久化结果，订单状态应保持在 make 之后的样子。
    let store = context.account_store.borrow();
    assert_eq!(
        token_amount(store.get(&maker_ata_a).expect("maker_ata_a 不存在")),
        500 - amount_a
    );
    assert_eq!(
        token_amount(store.get(&vault).expect("vault 不存在")),
        amount_a
    );
    assert!(store.get(&attacker_ata_a).is_none());
}
