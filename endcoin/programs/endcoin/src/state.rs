use anchor_lang::prelude::*;
#[account]
#[derive(Default)]
pub struct Amm {
    /// Account that has admin authority over the AMM
    pub admin: Pubkey,
    /// The LP fee taken on each trade, in basis points
    pub fee: u16,
    /// The AMM has been created
    pub created: bool,
}
impl Amm {
    pub const LEN: usize = 8 + 32 + 32 + 2 + 1;
}

#[account]
#[derive(Default)]
pub struct SST {
    /// temperature value in degrees celsius
    pub temperature: f64,
    pub created: bool,
}
impl SST {
    pub const LEN: usize = 8 + 8 + 1;
}

#[account()]
#[derive(Default)]
pub struct Pool {
    // Primary key of the AMM
    pub amm: Pubkey,
    /// Mint of token A - Endcoin
    pub mint_a: Pubkey,
    /// Mint of token B - Gaiacoin
    pub mint_b: Pubkey,
}
impl Pool {
    pub const LEN: usize = 8 + 32 + 32 + 32;
}

#[account()]
#[derive(Default)]
pub struct RewardVault {
    pub pool: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub bump: u8,
}
impl RewardVault {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 1;
}
