use core::mem::size_of;
use pinocchio::{
    account::{Ref, RefMut},
    error::ProgramError,
    AccountView, Address,
};
// #[repr(C)] 属性确保我们的结构体具有可预测的内存布局，这对于链上数据至关重要
#[repr(C)]
pub struct Escrow {
    seed: [u8; 8],    // 一个随机数，允许一个创建者使用相同的代币对创建多个托管
    maker: Address,   // 创建托管并接收代币的钱包地址
    mint_a: Address,  // 存入代币的SPL代币铸造地址
    mint_b: Address,  // 请求的代币的SPL代币铸造地址
    receive: [u8; 8], // 创建者希望接收的代币B的确切数量
    bump: [u8; 1],    // 在PDA推导中使用的单字节，用于确保地址不在Ed25519曲线上
}

impl Escrow {
    pub const LEN: usize = size_of::<Escrow>();

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

    #[inline(always)]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        &*(bytes.as_ptr() as *const Self)
    }
    #[inline(always)]
    unsafe fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut Self {
        &mut *(bytes.as_mut_ptr() as *mut Self)
    }
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
