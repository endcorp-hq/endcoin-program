use anchor_lang::prelude::*;

use crate::{
    constants::{DEATH_TEMP_C, END_RATE, FEE_BPS_DENOMINATOR, GAIA_RATE},
    errors::AmmError,
};

#[derive(Copy, Clone, Debug)]
pub enum EmissionTarget {
    Pool,
    Rewards,
}

#[derive(Debug)]
pub struct EmissionOutcome {
    pub amount_a: u64,
    pub amount_b: u64,
    pub liquidity: u64,
}

#[derive(Copy, Clone, Debug)]
pub struct TemperatureWeights {
    pub weight_end: f64,
    pub weight_gaia: f64,
}

pub fn calculate_emissions(mean_temp: f64, target: EmissionTarget) -> Result<EmissionOutcome> {
    require!(mean_temp.is_finite(), AmmError::InvalidTemperature);
    require!(
        mean_temp >= 0.0 && mean_temp <= DEATH_TEMP_C,
        AmmError::InvalidTemperature
    );

    // Exponential response to temperature mirrors the steepness near biological tipping points, then splits emissions by target.
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
    require!(
        mean_temp >= 0.0 && mean_temp <= DEATH_TEMP_C,
        AmmError::InvalidTemperature
    );

    // Higher temps increase Gaia emissions and reduce Endcoin; lower temps do the opposite.
    // We use exp-based reweighting to create a smooth but sensitive shift (same motivation as above).
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
    // Deterministic fee with a 1-unit minimum when fee_bps > 0 to prevent zero-fee trade splitting.
    let mut fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(AmmError::ArithmeticOverflow)?
        / FEE_BPS_DENOMINATOR as u128;
    if fee_bps > 0 && amount > 0 && fee == 0 {
        fee = 1;
    }
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
    // Weighted geometric mean invariant (Balancer-style constant mean market maker):
    // k = x^{w_end} * y^{w_gaia}; compare in log space to avoid overflow.
    // https://docs.balancer.fi/concepts/math/weighted-constant-mean-market-maker
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

    // Weighted swap output from Balancer's constant mean market maker:
    // out = r_out * (1 - (r_in / (r_in + dx))^{w_in/w_out})
    // https://docs.balancer.fi/concepts/math/weighted-constant-mean-market-maker#trades
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
    // Scaling to fixed precision keeps emissions stable on-chain without floating-point drift.
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
    // Liquidity mints use sqrt(x*y) (constant product) on integer amounts with overflow checks,
    // mirroring Uniswap V2's approach to LP token supply: https://docs.uniswap.org/contracts/v2/concepts/protocol-overview/how-uniswap-works
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

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::prelude::error::Error;

    fn code(err: Error) -> u32 {
        match err {
            Error::AnchorError(ae) => ae.error_code_number,
            Error::ProgramError(pe) => u64::from(pe.program_error) as u32,
        }
    }

    fn amm_code(err: AmmError) -> u32 {
        let code: u32 = err.into();
        code
    }

    fn approx_eq(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "left {} right {} tol {}", a, b, tol);
    }

    #[test]
    fn apply_fee_happy_path_and_invalid_bps() {
        let (net, fee) = apply_fee(5_000, 100).expect("fee applies");
        assert_eq!((net, fee), (4_950, 50));

        let err = apply_fee(1_000, FEE_BPS_DENOMINATOR + 1).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidFeeBps));
    }

    #[test]
    fn apply_fee_rounds_down_without_dust() {
        let (net, fee) = apply_fee(1, 333).expect("fee applies");
        assert_eq!((net, fee), (0, 1));

        let (net, fee) = apply_fee(10_001, 10_000).expect("max fee");
        assert_eq!((net, fee), (0, 10_001));
    }

    #[test]
    fn weighted_swap_matches_constant_product_when_weights_equal() {
        let amount_in = 1_000u64;
        let reserve_in = 500_000u64;
        let reserve_out = 750_000u64;

        let cp = ((amount_in as u128 * reserve_out as u128)
            / (reserve_in as u128 + amount_in as u128)) as u64;
        let weighted =
            compute_weighted_swap_output(amount_in, reserve_in, reserve_out, 0.5, 0.5).unwrap();

        let diff = (weighted as i128 - cp as i128).abs();
        assert!(
            diff <= 1,
            "weighted {} constant-product {} diff {}",
            weighted,
            cp,
            diff
        );
    }

    #[test]
    fn weighted_swap_respects_weight_bias() {
        let amount_in = 10_000;
        let reserve_in = 100_000;
        let reserve_out = 200_000;

        let bias_toward_input =
            compute_weighted_swap_output(amount_in, reserve_in, reserve_out, 0.8, 0.2).unwrap();
        let bias_toward_output =
            compute_weighted_swap_output(amount_in, reserve_in, reserve_out, 0.2, 0.8).unwrap();

        assert!(
            bias_toward_input > bias_toward_output,
            "higher weight_in/weight_out ratio should pay out more"
        );
    }

    #[test]
    fn weighted_swap_rejects_non_positive_weights() {
        let err = compute_weighted_swap_output(10, 100, 100, 0.0, 1.0).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::ArithmeticOverflow));
    }

    #[test]
    fn weighted_swap_rejects_zero_amount() {
        let err = compute_weighted_swap_output(0, 100, 100, 0.5, 0.5).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InputAmountTooSmall));
    }

    #[test]
    fn weighted_swap_rejects_zero_reserves() {
        let err = compute_weighted_swap_output(10, 0, 100, 0.5, 0.5).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InsufficientLiquidity));

        let err = compute_weighted_swap_output(10, 100, 0, 0.5, 0.5).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InsufficientLiquidity));
    }

    #[test]
    fn weighted_swap_rejects_non_finite_weights() {
        let err = compute_weighted_swap_output(10, 100, 100, f64::INFINITY, 1.0).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::ArithmeticOverflow));
    }

    #[test]
    fn weighted_invariant_tracks_weights() {
        let weights = TemperatureWeights {
            weight_end: 0.25,
            weight_gaia: 0.75,
        };
        let invariant = compute_weighted_invariant(1_000, 4_000, &weights).unwrap();
        let expected = 1_000f64.powf(0.25) * 4_000f64.powf(0.75);
        approx_eq(invariant, expected, 1e-6);
    }

    #[test]
    fn weighted_invariant_requires_liquidity() {
        let weights = TemperatureWeights {
            weight_end: 0.5,
            weight_gaia: 0.5,
        };
        let err = compute_weighted_invariant(0, 1, &weights).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InsufficientLiquidity));
    }

    #[test]
    fn weighted_invariant_rejects_non_finite_weights() {
        let weights = TemperatureWeights {
            weight_end: f64::NAN,
            weight_gaia: 1.0,
        };
        let err = compute_weighted_invariant(1, 1, &weights).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::ArithmeticOverflow));
    }

    #[test]
    fn temperature_weights_shift_with_temperature() {
        let low = temperature_weights(0.0).unwrap();
        let high = temperature_weights(35.0).unwrap();

        approx_eq(low.weight_end + low.weight_gaia, 1.0, 1e-12);
        approx_eq(high.weight_end + high.weight_gaia, 1.0, 1e-12);

        assert!(low.weight_end > low.weight_gaia);
        assert!(high.weight_gaia > high.weight_end);
    }

    #[test]
    fn temperature_weights_rejects_out_of_bounds() {
        let err = temperature_weights(-0.0001).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));

        let err = temperature_weights(DEATH_TEMP_C + 0.0001).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));

        // Bounds are inclusive.
        temperature_weights(0.0).expect("lower bound ok");
        temperature_weights(DEATH_TEMP_C).expect("upper bound ok");
    }

    #[test]
    fn temperature_weights_rejects_overflowing_exp() {
        let err = temperature_weights(1.0e9).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));

        let err = temperature_weights(-1.0e9).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));
    }

    #[test]
    fn temperature_weights_rejects_non_finite() {
        let err = temperature_weights(f64::NAN).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));
    }

    #[test]
    fn liquidity_is_integer_sqrt_of_product() {
        let liquidity = compute_liquidity(9, 16).unwrap();
        assert_eq!(liquidity, 12);
    }

    #[test]
    fn emissions_produce_positive_amounts() {
        let emissions = calculate_emissions(20.0, EmissionTarget::Pool).unwrap();
        assert!(emissions.amount_a > 0);
        assert!(emissions.amount_b > 0);
        assert!(emissions.liquidity > 0);
    }

    #[test]
    fn emissions_reject_out_of_bounds_temperature() {
        let err = calculate_emissions(-1.0, EmissionTarget::Pool).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));

        let err = calculate_emissions(DEATH_TEMP_C + 1.0, EmissionTarget::Rewards).unwrap_err();
        assert_eq!(code(err), amm_code(AmmError::InvalidTemperature));
    }

    #[test]
    fn reward_emissions_are_smaller_share_than_pool() {
        let pool = calculate_emissions(20.0, EmissionTarget::Pool).unwrap();
        let rewards = calculate_emissions(20.0, EmissionTarget::Rewards).unwrap();

        assert!(pool.amount_a > rewards.amount_a);
        assert!(pool.amount_b > rewards.amount_b);
        assert!(pool.liquidity > rewards.liquidity);
    }
}
