use anchor_lang::prelude::*;

use crate::{
    constants::{DEATH_TEMP_C, END_RATE, FEE_BPS_DENOMINATOR, GAIA_RATE},
    errors::AmmError,
};

#[derive(Copy, Clone)]
pub enum EmissionTarget {
    Pool,
    Rewards,
}

pub struct EmissionOutcome {
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity: u64,
}

pub struct TemperatureWeights {
    pub weight_end: f64,
    pub weight_gaia: f64,
}

pub fn calculate_emissions(mean_temp: f64, target: EmissionTarget) -> Result<EmissionOutcome> {
    require!(mean_temp.is_finite(), AmmError::InvalidTemperature);

    // Split the emission curve between pool liquidity and rewards while keeping total at 100%.
    let share_ratio = match target {
        EmissionTarget::Pool => 0.95_f64,
        EmissionTarget::Rewards => 0.05_f64,
    };

    // Exponential response ties issuance to deviation from the critical temperature line for each asset.
    let endcoin_raw = checked_exp((END_RATE * (DEATH_TEMP_C - mean_temp)) - 1.0)?;
    let gaiacoin_raw = checked_exp((GAIA_RATE * mean_temp) - 1.0)?;

    let amount_a = scaled_token_amount(endcoin_raw, share_ratio)?;
    let amount_b = scaled_token_amount(gaiacoin_raw, share_ratio)?;
    let liquidity = compute_liquidity(amount_a, amount_b)?;

    Ok(EmissionOutcome {
        amount_a,
        amount_b,
        liquidity,
    })
}

pub fn temperature_weights(mean_temp: f64) -> Result<TemperatureWeights> {
    require!(mean_temp.is_finite(), AmmError::InvalidTemperature);

    // Higher temps increase Gaia emissions and reduce Endcoin; lower temps do the opposite.
    let e_val = checked_exp((END_RATE * (DEATH_TEMP_C - mean_temp)) - 1.0)?;
    let g_val = checked_exp((GAIA_RATE * mean_temp) - 1.0)?;
    let total = e_val + g_val;
    require!(
        total.is_finite() && total > 0.0,
        AmmError::ArithmeticOverflow
    );

    Ok(TemperatureWeights {
        weight_end: e_val / total,
        weight_gaia: g_val / total,
    })
}

pub fn apply_fee(amount: u64, fee_bps: u16) -> Result<(u64, u64)> {
    require!(
        fee_bps as u64 <= FEE_BPS_DENOMINATOR as u64,
        AmmError::InvalidFeeBps
    );
    // Deterministic round-down fee to avoid dust creation and keep the invariant monotonic.
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(AmmError::ArithmeticOverflow)?
        / FEE_BPS_DENOMINATOR as u128;
    let fee = u64::try_from(fee).map_err(|_| AmmError::ArithmeticOverflow)?;
    require!(fee <= amount, AmmError::ArithmeticOverflow);

    Ok((amount - fee, fee))
}

pub fn compute_weighted_invariant(
    reserve_end: u64,
    reserve_gaia: u64,
    weights: &TemperatureWeights,
) -> Result<f64> {
    require!(
        reserve_end > 0 && reserve_gaia > 0,
        AmmError::InsufficientLiquidity
    );
    // k = x^{w_end} * y^{w_gaia}; compare in log space to avoid overflow.
    let k_log = weights.weight_end * (reserve_end as f64).ln()
        + weights.weight_gaia * (reserve_gaia as f64).ln();
    let k = ensure_finite(k_log.exp())?;
    require!(k > 0.0, AmmError::ArithmeticOverflow);
    Ok(k)
}

pub fn compute_weighted_swap_output(
    amount_in_after_fee: u64,
    reserve_in: u64,
    reserve_out: u64,
    weight_in: f64,
    weight_out: f64,
) -> Result<u64> {
    require!(amount_in_after_fee > 0, AmmError::InputAmountTooSmall);
    require!(
        reserve_in > 0 && reserve_out > 0,
        AmmError::InsufficientLiquidity
    );
    require!(
        weight_in > 0.0 && weight_out > 0.0,
        AmmError::ArithmeticOverflow
    );
    let weight_ratio = ensure_finite(weight_in / weight_out)?;
    require!(
        weight_ratio.is_sign_positive(),
        AmmError::ArithmeticOverflow
    );

    // out = r_out * (1 - (r_in / (r_in + dx))^{w_in/w_out})
    let base = ensure_finite((reserve_in as f64) / ((reserve_in + amount_in_after_fee) as f64))?;
    let power = ensure_finite(base.powf(weight_ratio))?;
    let output = ensure_finite((reserve_out as f64) * (1.0 - power))?;
    require!(
        output.is_finite() && output >= 0.0,
        AmmError::ArithmeticOverflow
    );

    u64::try_from(output.floor() as u128).map_err(|_| AmmError::ArithmeticOverflow.into())
}

fn scaled_token_amount(base_emission: f64, share_ratio: f64) -> Result<u64> {
    // Keep emissions non-negative, round to the nearest unit, then scale by 1_000 to match 3dp granularity.
    let scaled = ensure_finite(base_emission * share_ratio)?;
    require!(scaled >= 0.0, AmmError::ArithmeticOverflow);

    let rounded = scaled.round();
    require!(
        rounded <= (u64::MAX / 1000) as f64,
        AmmError::ArithmeticOverflow
    );
    let rounded = rounded as u64;

    rounded
        .checked_mul(1000)
        .ok_or(AmmError::ArithmeticOverflow.into())
}

fn checked_exp(value: f64) -> Result<f64> {
    ensure_finite(value.exp())
}

fn ensure_finite(value: f64) -> Result<f64> {
    require!(value.is_finite(), AmmError::ArithmeticOverflow);
    Ok(value)
}

fn compute_liquidity(amount_a: u64, amount_b: u64) -> Result<u64> {
    // Liquidity mints use sqrt(x*y) (constant product) on integer amounts with overflow checks.
    let product = (amount_a as u128)
        .checked_mul(amount_b as u128)
        .ok_or(AmmError::ArithmeticOverflow)?;
    let liquidity = integer_sqrt(product);
    u64::try_from(liquidity).map_err(|_| AmmError::ArithmeticOverflow.into())
}

fn integer_sqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    let mut x0 = value;
    let mut x1 = (x0 + value / x0) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + value / x0) / 2;
    }
    x0
}
