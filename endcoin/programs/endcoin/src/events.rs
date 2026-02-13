use anchor_lang::prelude::*;

#[event]
pub struct SwapEvent {
    pub trader: Pubkey,
    pub swap_a: bool,
    pub input_amount: u64,
    pub net_input_amount: u64,
    pub output_amount: u64,
    pub fee_bps: u16,
    pub temperature: f64,
    pub weight_end: f64,
    pub weight_gaia: f64,
    pub reserve_a: u64,
    pub reserve_b: u64,
}

#[event]
pub struct ClaimRewardEvent {
    pub claimer: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct DepositLiquidityEvent {
    pub pool: Pubkey,
    pub mean_temp: f64,
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity: u64,
}

#[event]
pub struct DepositRewardsEvent {
    pub reward_vault: Pubkey,
    pub mean_temp: f64,
    pub amount_a: u64,
    pub amount_b: u64,
}

#[event]
pub struct SstUpdatedEvent {
    pub feed: Pubkey,
    pub temperature: f64,
    pub slot: u64,
    pub unix_timestamp: i64,
}
