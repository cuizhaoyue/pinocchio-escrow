use core::mem::{align_of, size_of};
use pinocchio::{AccountView, Address, account::{Ref, RefMut}, error::ProgramError};

// ============================
// 新手说明（状态账户）
// Solana 账户本质是一段字节数组。
// 这里我们约定 escrow 账户的数据格式是 Escrow 结构体，并且长度固定。
//
// load/load_mut 的作用:
// - load:     从 &[u8] 零拷贝读取 Escrow 视图
// - load_mut: 从 &mut [u8] 零拷贝读取可变 Escrow 视图
//
// 为了让零拷贝更稳妥，这里让 Escrow 的对齐恒为 1（字段都按字节数组存储），
// 并用静态断言锁定 size/alignment。
// ============================

// Escrow 状态账户。这里使用零拷贝友好布局:
// - 用 `[u8; 8]` 存储 u64 字段，避免潜在未对齐读写问题
// - `#[repr(C)]` 保证字段顺序稳定
#[repr(C)]
pub struct Escrow {
    // 随机种子：用于与 maker 组合推导 escrow PDA。
    seed: [u8; 8],
    // 挂单方地址。
    maker: Address,
    // maker 存入代币的 mint（vault 对应的 mint）。
    mint_a: Address,
    // maker 期望收到代币的 mint。
    mint_b: Address,
    // 成交时 maker 期望收到的 token B 数量。
    receive: [u8; 8],
    // escrow PDA bump（单字节）。
    bump: [u8; 1],
}

impl Escrow {
    // 固定布局（无 padding）:
    // seed(8) + maker(32) + mint_a(32) + mint_b(32) + receive(8) + bump(1) = 113
    pub const LEN: usize = size_of::<Escrow>();

    // #[inline(always)]
    // fn check_len(len: usize) -> Result<(), ProgramError> {
    //     if len != Self::LEN {
    //         return Err(ProgramError::InvalidAccountData);
    //     }
    //     Ok(())
    // }

    #[inline(always)]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // SAFETY:
        // - `check_len` 已保证切片长度与 `Escrow::LEN` 一致
        // - `Escrow` 静态断言为 1 字节对齐，因此不存在未对齐解引用
        &*(bytes.as_ptr() as *const Self)
    }

    #[inline(always)]
    unsafe fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut Self {
        // SAFETY 条件同上；调用者通过 `load_mut` 进入时已过长度校验。
        &mut *(bytes.as_mut_ptr() as *mut Self)
    }

    #[inline(always)]
    pub fn load<'a>(account_view: &'a AccountView) -> Result<Ref<'a, Self>, ProgramError> {
        if account_view.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(Ref::map(account_view.try_borrow()?, |data| unsafe {
            Self::from_bytes_unchecked(data)
        }))
    }

    #[inline(always)]
    pub fn load_mut<'a>(account_view: &'a AccountView) -> Result<RefMut<'a, Self>, ProgramError> {
        if account_view.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(RefMut::map(account_view.try_borrow_mut()?, |data| unsafe {
            Self::from_bytes_unchecked_mut(data)
        }))
    }

    // #[inline(always)]
    // pub fn load(bytes: &[u8]) -> Result<&Self, ProgramError> {
    //     Self::check_len(bytes.len())?;
    //     Ok(unsafe { Self::from_bytes_unchecked(bytes) })
    // }

    // #[inline(always)]
    // pub fn load_mut(bytes: &mut [u8]) -> Result<&mut Self, ProgramError> {
    //     Self::check_len(bytes.len())?;
    //     Ok(unsafe { Self::from_bytes_unchecked_mut(bytes) })
    // }

    #[inline(always)]
    pub fn seed(&self) -> u64 {
        u64::from_le_bytes(self.seed)
    }

    #[inline(always)]
    pub fn maker(&self) -> &Address {
        &self.maker
    }

    #[inline(always)]
    pub fn mint_a(&self) -> &Address {
        &self.mint_a
    }

    #[inline(always)]
    pub fn mint_b(&self) -> &Address {
        &self.mint_b
    }

    #[inline(always)]
    pub fn receive(&self) -> u64 {
        u64::from_le_bytes(self.receive)
    }

    #[inline(always)]
    pub fn bump(&self) -> [u8; 1] {
        self.bump
    }

    #[inline(always)]
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_maker(&mut self, maker: Address) {
        self.maker = maker;
    }

    #[inline(always)]
    pub fn set_mint_a(&mut self, mint_a: Address) {
        self.mint_a = mint_a;
    }

    #[inline(always)]
    pub fn set_mint_b(&mut self, mint_b: Address) {
        self.mint_b = mint_b;
    }

    #[inline(always)]
    pub fn set_receive(&mut self, receive: u64) {
        self.receive = receive.to_le_bytes();
    }

    #[inline(always)]
    pub fn set_bump(&mut self, bump: [u8; 1]) {
        self.bump = bump;
    }

    #[inline(always)]
    pub fn set_inner(
        &mut self,
        seed: u64,
        maker: Address,
        mint_a: Address,
        mint_b: Address,
        receive: u64,
        bump: [u8; 1],
    ) {
        self.set_seed(seed);
        self.set_maker(maker);
        self.set_mint_a(mint_a);
        self.set_mint_b(mint_b);
        self.set_receive(receive);
        self.set_bump(bump);
    }
}

const _: [(); 113] = [(); size_of::<Escrow>()];
const _: [(); 1] = [(); align_of::<Escrow>()];
