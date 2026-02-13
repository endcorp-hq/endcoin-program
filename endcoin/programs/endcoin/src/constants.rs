use anchor_lang::prelude::*;

#[constant]
pub const POOL_AUTHORITY_SEED: &[u8] = b"pool-authority";
#[constant]
pub const REWARD_VAULT_SEED: &[u8] = b"reward-vault";
#[constant]
pub const SST_SEED: &[u8] = b"sea-surface-temperature";
#[constant]
pub const AMM_SEED: &[u8] = b"amm";
#[constant]
pub const FEE_BPS_DENOMINATOR: u16 = 10_000; // 100%
#[constant]
pub const MIN_FEE_BPS: u16 = 5;
#[constant]
pub const MAX_FEE_BPS: u16 = 1_000; // 10%
#[constant]
pub const PARAM_UPDATE_COOLDOWN_SLOTS: u64 = 1;
#[constant]
pub const MAX_ORACLE_STALENESS_SECONDS: i64 = 15 * 60;
#[constant]
pub const MAX_SWAP_OUTPUT_BPS: u16 = 3_000; // 30%

// Emission curve parameters
pub const DEATH_TEMP_C: f64 = 35.0;
pub const END_RATE: f64 = 1.125;
pub const GAIA_RATE: f64 = 0.750;
