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
    /// Slot where fee was most recently changed.
    pub last_fee_update_slot: u64,
}
impl Amm {
    pub const LEN: usize = 8 + 32 + 2 + 1 + 8;
}

#[account]
#[derive(Default)]
pub struct SST {
    /// temperature value in degrees celsius
    pub temperature: f64,
    pub created: bool,
    /// The Switchboard feed account used for SST.
    pub oracle_feed: Pubkey,
    /// Slot/time of the most recent oracle pull.
    pub last_updated_slot: u64,
    pub last_updated_unix_timestamp: i64,
}
impl SST {
    pub const LEN: usize = 8 + 8 + 1 + 32 + 8 + 8;
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
    /// Canonical reserve account for mint_a.
    pub reserve_a: Pubkey,
    /// Canonical reserve account for mint_b.
    pub reserve_b: Pubkey,
}
impl Pool {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 32 + 32;
}

#[account()]
#[derive(Default)]
pub struct RewardVault {
    pub pool: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub bump: u8,
    /// External signer that approves reward claims.
    pub whitelist_authority: Pubkey,
}
impl RewardVault {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 1 + 32;
}
