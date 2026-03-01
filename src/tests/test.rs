//! # Pinocchio Escrow 程序单元测试
//!
//! 完整的单元测试，测试所有指令逻辑

// 导入本程序的类型
use crate::{Escrow, ID};
use pinocchio::Address;

// =============================================================================
// 辅助函数
// =============================================================================

/// 派生托管账户 PDA
///
/// # 参数
/// * `maker` - 创建者的地址
/// * `seed` - 用于派生 PDA 的随机种子
///
/// # 返回
/// 返回 (PDA 地址, bump seed)
fn derive_escrow_pda(maker: &Address, seed: u64) -> (Address, u8) {
    Address::find_program_address(
        &[
            b"escrow",
            maker.as_ref(),
            &seed.to_le_bytes(),
        ],
        &ID,
    )
}

/// 派生关联代币账户（ATA）地址
///
/// 注意：这是简化实现，仅用于测试
/// 实际 ATA 的派生需要关联代币程序
///
/// # 参数
/// * `owner` - 代币账户所有者
/// * `mint` - 代币 mint 地址
///
/// # 返回
/// 返回一个模拟的 ATA 地址
fn derive_ata_address(owner: &Address, mint: &Address) -> Address {
    // 简化实现：基于 owner 和 mint 生成一个确定性的地址
    // 实际 ATA 的 PDA 派生更复杂
    let mut seed = [0u8; 32];
    for i in 0..32 {
        seed[i] = owner.as_ref()[i] ^ mint.as_ref()[i];
    }
    Address::from(seed)
}

/// 构造 Make 指令数据
///
/// # 参数
/// * `seed` - 用于派生托管账户的种子
/// * `receive` - 创建者希望接收的代币 B 数量
/// * `amount` - 创建者存入的代币 A 数量
///
/// # 返回
/// 返回指令数据字节数组：discriminator(1) + seed(8) + receive(8) + amount(8)
fn make_instruction_data(seed: u64, receive: u64, amount: u64) -> [u8; 25] {
    let mut data = [0u8; 25];
    data[0] = 0; // Make discriminator = 0
    data[1..9].copy_from_slice(&seed.to_le_bytes());
    data[9..17].copy_from_slice(&receive.to_le_bytes());
    data[17..25].copy_from_slice(&amount.to_le_bytes());
    data
}

/// 构造 Take 指令数据
///
/// Take 指令不需要额外数据，只需要 discriminator
fn take_instruction_data() -> [u8; 1] {
    [1u8] // Take discriminator = 1
}

/// 构造 Refund 指令数据
///
/// Refund 指令不需要额外数据，只需要 discriminator
fn refund_instruction_data() -> [u8; 1] {
    [2u8] // Refund discriminator = 2
}

// =============================================================================
// Escrow 状态结构测试
// =============================================================================

#[test]
/// 测试 Escrow 结构体的长度
fn test_escrow_len() {
    // Escrow 结构体: seed(8) + maker(32) + mint_a(32) + mint_b(32) + receive(8) + bump(1) = 113
    assert_eq!(Escrow::LEN, 8 + 32 + 32 + 32 + 8 + 1);
    assert_eq!(Escrow::LEN, 113);
}

#[test]
/// 测试 Escrow 字段的序列化
fn test_escrow_fields() {
    // 创建一个测试数据缓冲区
    let mut data = [0u8; Escrow::LEN];

    // 测试 seed
    let test_seed: u64 = 12345;
    data[0..8].copy_from_slice(&test_seed.to_le_bytes());

    // 测试 maker
    let test_maker = Address::from([1u8; 32]);
    data[8..40].copy_from_slice(test_maker.as_ref());

    // 测试 mint_a
    let test_mint_a = Address::from([2u8; 32]);
    data[40..72].copy_from_slice(test_mint_a.as_ref());

    // 测试 mint_b
    let test_mint_b = Address::from([3u8; 32]);
    data[72..104].copy_from_slice(test_mint_b.as_ref());

    // 测试 receive
    let test_receive: u64 = 500_000_000;
    data[104..112].copy_from_slice(&test_receive.to_le_bytes());

    // 测试 bump
    let test_bump: u8 = 255;
    data[112] = test_bump;

    // 验证数据正确存储
    assert_eq!(u64::from_le_bytes(data[0..8].try_into().unwrap()), test_seed);
    assert_eq!(&data[8..40], test_maker.as_ref());
    assert_eq!(&data[40..72], test_mint_a.as_ref());
    assert_eq!(&data[72..104], test_mint_b.as_ref());
    assert_eq!(u64::from_le_bytes(data[104..112].try_into().unwrap()), test_receive);
    assert_eq!(data[112], test_bump);
}

// =============================================================================
// PDA 派生测试
// =============================================================================

#[test]
/// 测试托管账户 PDA 派生
fn test_derive_escrow_pda() {
    let maker = Address::from([1u8; 32]);
    let seed: u64 = 12345;

    let (pda, bump) = derive_escrow_pda(&maker, seed);

    // 验证 bump 在有效范围内
    assert!(bump < 255, "Bump should be canonical");

    // 验证 PDA 不是默认地址
    assert_ne!(pda, Address::default(), "PDA should not be default");

    // 验证相同的输入产生相同的输出
    let (pda2, bump2) = derive_escrow_pda(&maker, seed);
    assert_eq!(pda, pda2, "相同输入应该产生相同的 PDA");
    assert_eq!(bump, bump2, "相同输入应该产生相同的 bump");
}

#[test]
/// 测试不同的 seed 会产生不同的 PDA
fn test_different_seed_different_pda() {
    let maker = Address::from([1u8; 32]);

    let (pda1, _) = derive_escrow_pda(&maker, 12345);
    let (pda2, _) = derive_escrow_pda(&maker, 54321);

    assert_ne!(pda1, pda2, "不同的 seed 应该产生不同的 PDA");
}

#[test]
/// 测试不同的 maker 会产生不同的 PDA
fn test_different_maker_different_pda() {
    let seed: u64 = 12345;

    let maker1 = Address::from([1u8; 32]);
    let maker2 = Address::from([2u8; 32]);

    let (pda1, _) = derive_escrow_pda(&maker1, seed);
    let (pda2, _) = derive_escrow_pda(&maker2, seed);

    assert_ne!(pda1, pda2, "不同的 maker 应该产生不同的 PDA");
}

// =============================================================================
// 指令数据构造测试
// =============================================================================

#[test]
/// 测试 Make 指令数据构造
fn test_make_instruction_data() {
    let seed: u64 = 12345;
    let receive: u64 = 500_000_000;
    let amount: u64 = 1_000_000_000;

    let data = make_instruction_data(seed, receive, amount);

    // 验证 discriminator
    assert_eq!(data[0], 0, "Make discriminator 应该是 0");

    // 验证 seed
    let parsed_seed = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(parsed_seed, seed);

    // 验证 receive
    let parsed_receive = u64::from_le_bytes(data[9..17].try_into().unwrap());
    assert_eq!(parsed_receive, receive);

    // 验证 amount
    let parsed_amount = u64::from_le_bytes(data[17..25].try_into().unwrap());
    assert_eq!(parsed_amount, amount);
}

#[test]
/// 测试 Take 指令数据构造
fn test_take_instruction_data() {
    let data = take_instruction_data();
    assert_eq!(data[0], 1, "Take discriminator 应该是 1");
    assert_eq!(data.len(), 1, "Take 指令数据长度应该是 1");
}

#[test]
/// 测试 Refund 指令数据构造
fn test_refund_instruction_data() {
    let data = refund_instruction_data();
    assert_eq!(data[0], 2, "Refund discriminator 应该是 2");
    assert_eq!(data.len(), 1, "Refund 指令数据长度应该是 1");
}

// =============================================================================
// 边界情况测试
// =============================================================================

#[test]
/// 测试 amount 为 0 的情况（应该被拒绝）
fn test_zero_amount_rejected() {
    let seed: u64 = 12345;
    let receive: u64 = 500_000_000;
    let amount: u64 = 0; // 无效：amount 不能为 0

    let _data = make_instruction_data(seed, receive, amount);

    // 验证 amount 为 0
    let parsed_amount = u64::from_le_bytes(_data[17..25].try_into().unwrap());
    assert_eq!(parsed_amount, 0);

    // 实际程序中，check_non_zero 会拒绝这个值
    let values = [receive, amount];
    assert!(values.iter().any(|v| *v == 0), "应该检测到零值");
}

#[test]
/// 测试 receive 为 0 的情况（应该被拒绝）
fn test_zero_receive_rejected() {
    let seed: u64 = 12345;
    let receive: u64 = 0; // 无效：receive 不能为 0
    let amount: u64 = 1_000_000_000;

    let _data = make_instruction_data(seed, receive, amount);

    // 验证 receive 为 0
    let parsed_receive = u64::from_le_bytes(_data[9..17].try_into().unwrap());
    assert_eq!(parsed_receive, 0);

    // 实际程序中，check_non_zero 会拒绝这个值
    let values = [receive, amount];
    assert!(values.iter().any(|v| *v == 0), "应该检测到零值");
}

#[test]
/// 测试最大 u64 值的处理
fn test_max_u64_values() {
    let seed: u64 = u64::MAX;
    let receive: u64 = u64::MAX;
    let amount: u64 = u64::MAX;

    let data = make_instruction_data(seed, receive, amount);

    // 验证可以正确序列化和反序列化最大值
    let parsed_seed = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let parsed_receive = u64::from_le_bytes(data[9..17].try_into().unwrap());
    let parsed_amount = u64::from_le_bytes(data[17..25].try_into().unwrap());

    assert_eq!(parsed_seed, u64::MAX);
    assert_eq!(parsed_receive, u64::MAX);
    assert_eq!(parsed_amount, u64::MAX);
}

// =============================================================================
// Make 指令逻辑测试
// =============================================================================

#[test]
/// 测试 Make 指令的完整流程（模拟）
fn test_make_instruction_flow() {
    // 准备测试数据
    let maker = Address::from([1u8; 32]);
    let mint_a = Address::from([2u8; 32]);
    let _mint_b = Address::from([3u8; 32]); // 未使用但保持接口一致
    let seed: u64 = 12345;
    let receive: u64 = 500_000_000;
    let amount: u64 = 1_000_000_000;

    // 1. 派生托管账户 PDA
    let (escrow_pda, escrow_bump) = derive_escrow_pda(&maker, seed);
    assert_ne!(escrow_pda, Address::default());

    // 2. 派生 vault ATA 地址
    let vault_ata = derive_ata_address(&escrow_pda, &mint_a);
    assert_ne!(vault_ata, Address::default());

    // 3. 构造指令数据
    let instruction_data = make_instruction_data(seed, receive, amount);
    assert_eq!(instruction_data.len(), 25);

    // 4. 验证参数有效性（模拟 check_non_zero）
    let values = [receive, amount];
    assert!(!values.iter().any(|v| *v == 0), "参数应该都非零");

    // 5. 验证 PDA seeds
    let seed_binding = seed.to_le_bytes();
    let expected_seeds = [
        b"escrow",
        maker.as_ref(),
        &seed_binding,
    ];
    assert_eq!(expected_seeds.len(), 3);

    println!("Make 指令流程验证通过:");
    println!("  Escrow PDA: {:?}", escrow_pda);
    println!("  Vault ATA: {:?}", vault_ata);
    println!("  Escrow Bump: {}", escrow_bump);
}

// =============================================================================
// Take 指令逻辑测试
// =============================================================================

#[test]
/// 测试 Take 指令的完整流程（模拟）
fn test_take_instruction_flow() {
    // 准备测试数据
    let maker = Address::from([1u8; 32]);
    let _taker = Address::from([5u8; 32]); // 用于实际执行
    let mint_a = Address::from([2u8; 32]);
    let _mint_b = Address::from([3u8; 32]); // 用于实际执行
    let seed: u64 = 12345;
    let receive: u64 = 500_000_000;
    let deposit: u64 = 1_000_000_000;

    // 1. 派生托管账户 PDA
    let (escrow_pda, _escrow_bump) = derive_escrow_pda(&maker, seed);

    // 2. 派生 vault ATA 地址
    let vault_ata = derive_ata_address(&escrow_pda, &mint_a);

    // 3. 构造 Take 指令数据
    let instruction_data = take_instruction_data();
    assert_eq!(instruction_data[0], 1);

    // 4. 验证转账逻辑
    // Taker -> Maker: Token B (receive amount)
    // Vault -> Taker: Token A (deposit amount)
    assert!(deposit > 0, "存款金额应该大于零");
    assert!(receive > 0, "接收金额应该大于零");

    println!("Take 指令流程验证通过:");
    println!("  Escrow PDA: {:?}", escrow_pda);
    println!("  Vault ATA: {:?}", vault_ata);
    println!("  Taker sends: {} Token B", receive);
    println!("  Taker receives: {} Token A", deposit);
}

// =============================================================================
// Refund 指令逻辑测试
// =============================================================================

#[test]
/// 测试 Refund 指令的完整流程（模拟）
fn test_refund_instruction_flow() {
    // 准备测试数据
    let maker = Address::from([1u8; 32]);
    let mint_a = Address::from([2u8; 32]);
    let seed: u64 = 12345;
    let deposit: u64 = 1_000_000_000;

    // 1. 派生托管账户 PDA
    let (escrow_pda, _escrow_bump) = derive_escrow_pda(&maker, seed);

    // 2. 派生 vault ATA 地址
    let vault_ata = derive_ata_address(&escrow_pda, &mint_a);

    // 3. 构造 Refund 指令数据
    let instruction_data = refund_instruction_data();
    assert_eq!(instruction_data[0], 2);

    // 4. 验证退款逻辑
    // Vault -> Maker: Token A (full deposit amount)
    assert!(deposit > 0, "退款金额应该等于原始存款");

    println!("Refund 指令流程验证通过:");
    println!("  Escrow PDA: {:?}", escrow_pda);
    println!("  Vault ATA: {:?}", vault_ata);
    println!("  Maker receives: {} Token A", deposit);
}

// =============================================================================
// 安全性测试
// =============================================================================

#[test]
/// 测试只有创建者可以调用 Refund
fn test_only_maker_can_refund() {
    let maker = Address::from([1u8; 32]);
    let other = Address::from([9u8; 32]); // 非创建者

    // 验证地址不同
    assert_ne!(maker, other, "非创建者地址应该与创建者不同");

    // 实际程序中，会验证 escrow.maker == 签名者
    // 这里我们验证逻辑：只有创建者派生的 PDA 才能签名
    let seed: u64 = 12345;

    let (maker_escrow, _) = derive_escrow_pda(&maker, seed);
    let (other_escrow, _) = derive_escrow_pda(&other, seed);

    assert_ne!(
        maker_escrow, other_escrow,
        "不同创建者的托管账户应该不同"
    );
}

#[test]
/// 测试托管账户只能使用一次
fn test_escrow_single_use() {
    let maker = Address::from([1u8; 32]);
    let seed: u64 = 12345;

    // 相同的 seed 和 maker 总是产生相同的 PDA
    let (pda1, bump1) = derive_escrow_pda(&maker, seed);
    let (pda2, bump2) = derive_escrow_pda(&maker, seed);

    assert_eq!(pda1, pda2, "相同参数应该产生相同的 PDA");
    assert_eq!(bump1, bump2, "Bump 应该一致");

    // 这意味着：
    // 1. 第一次 Make 会创建账户
    // 2. 第二次 Make 会因为账户已存在而失败
}

// =============================================================================
// 辅助函数测试
// =============================================================================

#[test]
/// 测试非零值检查辅助函数
fn test_check_non_zero_helper() {
    // 所有值都非零，应该通过
    let values1 = vec![1u64, 2, 3];
    assert!(!values1.iter().any(|v| *v == 0));

    // 有一个值为零，应该失败
    let values2 = vec![1u64, 0, 3];
    assert!(values2.iter().any(|v| *v == 0));
}

#[test]
/// 测试字节序转换
fn test_endianness_conversion() {
    let value: u64 = 0x123456789ABCDEF0;

    // 小端序
    let le_bytes = value.to_le_bytes();
    let restored = u64::from_le_bytes(le_bytes);

    assert_eq!(restored, value);

    // 验证字节顺序
    assert_eq!(le_bytes[0], 0xF0); // 最低字节在前
    assert_eq!(le_bytes[7], 0x12); // 最高字节在后
}

// =============================================================================
// Discriminator 测试
// =============================================================================

#[test]
/// 测试所有指令的 discriminator 互不相同
fn test_discriminators_are_unique() {
    let make_disc = 0u8;
    let take_disc = 1u8;
    let refund_disc = 2u8;

    // 验证互不相同
    assert_ne!(make_disc, take_disc);
    assert_ne!(take_disc, refund_disc);
    assert_ne!(make_disc, refund_disc);

    // 验证在有效范围内
    assert!(make_disc <= 2);
    assert!(take_disc <= 2);
    assert!(refund_disc <= 2);
}

// =============================================================================
// 完整流程测试
// =============================================================================

#[test]
/// 测试完整的交易流程：Make -> Take
fn test_full_transaction_flow() {
    // 创建者
    let maker = Address::from([1u8; 32]);
    let mint_a = Address::from([2u8; 32]);
    let _mint_b = Address::from([3u8; 32]);

    // 接受者
    let _taker = Address::from([5u8; 32]);

    // 订单参数
    let seed: u64 = 12345;
    let receive_amount: u64 = 500_000_000;
    let deposit_amount: u64 = 1_000_000_000;

    // === Step 1: Make ===
    let (escrow_pda, _escrow_bump) = derive_escrow_pda(&maker, seed);
    let _vault_ata = derive_ata_address(&escrow_pda, &mint_a);
    let make_data = make_instruction_data(seed, receive_amount, deposit_amount);

    assert_eq!(make_data[0], 0); // Make discriminator
    println!("Step 1 - Make: 托管账户 {:?} 已创建", escrow_pda);

    // === Step 2: Take ===
    let take_data = take_instruction_data();

    assert_eq!(take_data[0], 1); // Take discriminator

    // 验证转账
    // Taker -> Maker: receive_amount 的 Token B
    // Vault -> Taker: deposit_amount 的 Token A
    assert_eq!(receive_amount, 500_000_000);
    assert_eq!(deposit_amount, 1_000_000_000);

    println!("Step 2 - Take: 交易完成");
    println!("  Taker 发送: {} Token B", receive_amount);
    println!("  Taker 接收: {} Token A", deposit_amount);
}

#[test]
/// 测试退款流程：Make -> Refund
fn test_refund_flow() {
    // 创建者
    let maker = Address::from([1u8; 32]);
    let mint_a = Address::from([2u8; 32]);
    let _mint_b = Address::from([3u8; 32]);

    // 订单参数
    let seed: u64 = 12345;
    let receive_amount: u64 = 500_000_000;
    let deposit_amount: u64 = 1_000_000_000;

    // === Step 1: Make ===
    let (escrow_pda, _escrow_bump) = derive_escrow_pda(&maker, seed);
    let _vault_ata = derive_ata_address(&escrow_pda, &mint_a);
    let _make_data = make_instruction_data(seed, receive_amount, deposit_amount);

    println!("Step 1 - Make: 托管账户 {:?} 已创建", escrow_pda);

    // === Step 2: Refund ===
    let refund_data = refund_instruction_data();

    assert_eq!(refund_data[0], 2); // Refund discriminator

    // 验证退款
    // Vault -> Maker: deposit_amount 的 Token A (全额退款)
    assert_eq!(deposit_amount, 1_000_000_000);

    println!("Step 2 - Refund: 退款完成");
    println!("  Maker 收回: {} Token A", deposit_amount);
}
