# Pinocchio Escrow (Solana)

基于 `pinocchio` 开发的托管合约示例，实现了一个最小可用的 Escrow 交易流程：

- `make`：maker 挂单，锁定 token A 到 vault
- `take`：taker 吃单，支付 token B，换走 token A
- `refund`：maker 撤单，取回 token A

挑战地址：<https://learn.blueshift.gg/zh-CN/challenges/pinocchio-escrow>

## 程序信息

- Package: `pinocchio-escrow`
- SBF 程序名: `pinocchio_escrow`
- Program ID: `22222222222222222222222222222222222222222222`
- 入口: `src/lib.rs` 中 `process_instruction`

## 指令定义

### 1) make

- discriminator: `0`
- data 格式: `[seed:u64 | receive:u64 | amount:u64]`（均为 little-endian）
- 账户顺序:
1. `maker`（signer）
2. `escrow`（PDA，待创建）
3. `mint_a`
4. `mint_b`
5. `maker_ata_a`
6. `vault`（escrow 的 ATA）
7. `system_program`
8. `token_program`
9. `associated_token_program`（占位，当前代码会接收）

### 2) take

- discriminator: `1`
- data 格式: 无额外数据
- 账户顺序:
1. `taker`（signer）
2. `maker`
3. `escrow`
4. `mint_a`
5. `mint_b`
6. `vault`
7. `taker_ata_a`
8. `taker_ata_b`
9. `maker_ata_b`
10. `system_program`
11. `token_program`
12. `associated_token_program`（占位，当前代码会接收）

### 3) refund

- discriminator: `2`
- data 格式: 无额外数据
- 账户顺序:
1. `maker`（signer）
2. `escrow`
3. `mint_a`
4. `maker_ata_a`
5. `vault`
6. `system_program`
7. `token_program`
8. `associated_token_program`（可传可不传，代码按前 7 个必需账户处理）

## Escrow 状态结构

`src/state.rs` 中 `Escrow` 的固定长度为 `113` 字节：

- `seed`: 8
- `maker`: 32
- `mint_a`: 32
- `mint_b`: 32
- `receive`: 8
- `bump`: 1

## 本地开发

### 依赖

- Rust stable
- Solana SBF 工具链（用于产出 `.so`，Mollusk 测试会加载）

### 构建

```bash
cargo build
```

### 运行测试（Mollusk）

先构建 SBF 程序，再跑单测：

```bash
cargo build-sbf
cargo test
```

说明：

- 测试模块在 `src/tests/mollusk_unit.rs` 和 `src/tests/mollusk_flows.rs`
- 流程测试覆盖了 `make/take/refund` 的真实执行，不是纯模拟断言

## 常见问题

### 1) 找不到 `.so` 文件

如果报错类似：

`未找到程序文件 target/deploy/pinocchio_escrow.so`

请先执行：

```bash
cargo build-sbf
```

### 2) 指令数据错误

`make` 必须携带 24 字节参数（3 个 `u64`），并且 `receive` 与 `amount` 都不能为 `0`，否则会返回 `InvalidInstructionData`。

### 3) 账户顺序错误

如果客户端传参顺序和上面定义不一致，常见报错是：

- `NotEnoughAccountKeys`
- `InvalidAccountData`
- 自定义错误码（如 `InvalidAddress` / `InvalidMaker`）
