use anchor_lang::prelude::*;

#[constant]
pub const POOL_AUTHORITY_SEED: &[u8] = b"pool-authority";
#[constant]
pub const REWARD_VAULT_SEED: &[u8] = b"reward-vault";
#[constant]
pub const AUTHORITY_SEED: &[u8] = b"authority";
#[constant]
pub const SST_SEED: &[u8] = b"sea-surface-temperature";
#[constant]
pub const AMM_SEED: &[u8] = b"amm";

#[constant]
pub const FEE_BPS_DENOMINATOR: u16 = 10_000; // 100%