use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::{
        AMM_SEED, FEE_BPS_DENOMINATOR, MAX_ORACLE_STALENESS_SECONDS, MAX_SWAP_OUTPUT_BPS,
        PARAM_UPDATE_COOLDOWN_SLOTS, POOL_AUTHORITY_SEED, SST_SEED,
    },
    errors::*,
    events::SwapEvent,
    math::{
        apply_fee, compute_weighted_invariant, compute_weighted_swap_output, temperature_weights,
    },
    state::{Amm, Pool, SST},
};

fn enforce_recent_parameters(amm: &Amm, sst: &SST) -> Result<()> {
    require!(sst.created, AmmError::InvalidTemperature);
    require!(
        sst.last_updated_unix_timestamp > 0,
        AmmError::OracleValueMissing
    );

    let clock = Clock::get()?;

    let oracle_staleness = clock
        .unix_timestamp
        .checked_sub(sst.last_updated_unix_timestamp)
        .ok_or(AmmError::OracleFeedStale)?;
    require!(
        oracle_staleness <= MAX_ORACLE_STALENESS_SECONDS,
        AmmError::OracleFeedStale
    );

    let sst_ready_slot = sst
        .last_updated_slot
        .checked_add(PARAM_UPDATE_COOLDOWN_SLOTS)
        .ok_or(AmmError::ArithmeticOverflow)?;
    require!(
        clock.slot >= sst_ready_slot,
        AmmError::ParameterRecentlyUpdated
    );

    let fee_ready_slot = amm
        .last_fee_update_slot
        .checked_add(PARAM_UPDATE_COOLDOWN_SLOTS)
        .ok_or(AmmError::ArithmeticOverflow)?;
    require!(
        clock.slot >= fee_ready_slot,
        AmmError::ParameterRecentlyUpdated
    );

    Ok(())
}

#[derive(Accounts)]
pub struct SwapExactTokensForTokens<'info> {
    #[account(seeds = [AMM_SEED], bump)]
    pub amm: Account<'info, Amm>,

    /// CHECK: Read only authority
    #[account(
        mut,
        seeds = [
            amm.key().as_ref(),
            mint_a.key().as_ref(),
            mint_b.key().as_ref(),
            POOL_AUTHORITY_SEED,
        ],
        bump,
    )]
    pub pool_authority: AccountInfo<'info>,

    /// The account doing the swap
    pub trader: Signer<'info>,

    pub mint_a: Box<InterfaceAccount<'info, Mint>>,
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        seeds = [
            pool.amm.as_ref(),
            pool.mint_a.key().as_ref(),
            pool.mint_b.key().as_ref(),
        ],
        bump,
        has_one = amm,
        has_one = mint_a,
        has_one = mint_b,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        mut,
        address = pool.reserve_a @ AmmError::InvalidPoolAccount,
        constraint = pool_account_a.owner == pool_authority.key() @ AmmError::InvalidPoolAccount,
        constraint = pool_account_a.mint == mint_a.key() @ AmmError::InvalidPoolAccount,
    )]
    pub pool_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        address = pool.reserve_b @ AmmError::InvalidPoolAccount,
        constraint = pool_account_b.owner == pool_authority.key() @ AmmError::InvalidPoolAccount,
        constraint = pool_account_b.mint == mint_b.key() @ AmmError::InvalidPoolAccount,
    )]
    pub pool_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [SST_SEED, amm.key().as_ref()],
        bump,
    )]
    pub sst: Box<Account<'info, SST>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = trader,
        associated_token::token_program = token_program,
    )]
    pub trader_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = trader,
        associated_token::token_program = token_program,
    )]
    pub trader_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The account paying for rent
    #[account(mut)]
    pub payer: Signer<'info>,

    // Solana accounts
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> SwapExactTokensForTokens<'info> {
    pub fn swap(
        &mut self,
        swap_a: bool,
        input_amount: u64,
        min_output_amount: u64,
        bumps: &SwapExactTokensForTokensBumps,
    ) -> Result<()> {
        require!(input_amount > 0, AmmError::InputAmountTooSmall);

        enforce_recent_parameters(&self.amm, &self.sst)?;

        // Temperature-aware weights tilt pricing based on the latest oracle SST.
        let weights = temperature_weights(self.sst.temperature)?;

        let (
            input_account,
            output_account,
            input_pool_account,
            output_pool_account,
            input_mint,
            output_mint,
        ) = if swap_a {
            (
                &self.trader_account_a,
                &self.trader_account_b,
                &self.pool_account_a,
                &self.pool_account_b,
                &self.mint_a,
                &self.mint_b,
            )
        } else {
            (
                &self.trader_account_b,
                &self.trader_account_a,
                &self.pool_account_b,
                &self.pool_account_a,
                &self.mint_b,
                &self.mint_a,
            )
        };

        require!(
            input_amount <= input_account.amount,
            AmmError::InsufficientBalance
        );

        let (reserve_in, reserve_out) = (input_pool_account.amount, output_pool_account.amount);

        let invariant = compute_weighted_invariant(
            self.pool_account_a.amount,
            self.pool_account_b.amount,
            &weights,
        )?;

        let (taxed_input, _) = apply_fee(input_amount, self.amm.fee)?;
        require!(taxed_input > 0, AmmError::InputAmountTooSmall);

        let (weight_in, weight_out) = if swap_a {
            (weights.weight_end, weights.weight_gaia)
        } else {
            (weights.weight_gaia, weights.weight_end)
        };

        let output = compute_weighted_swap_output(
            taxed_input,
            reserve_in,
            reserve_out,
            weight_in,
            weight_out,
        )?;
        require!(output >= min_output_amount, AmmError::OutputTooSmall);

        let output_share_bps = (output as u128)
            .checked_mul(FEE_BPS_DENOMINATOR as u128)
            .ok_or(AmmError::ArithmeticOverflow)?
            / reserve_out as u128;
        require!(
            output_share_bps <= MAX_SWAP_OUTPUT_BPS as u128,
            AmmError::PriceImpactTooHigh
        );

        let authority_bump = bumps.pool_authority;
        let authority_seeds = &[
            self.pool.amm.as_ref(),
            self.pool.mint_a.as_ref(),
            self.pool.mint_b.as_ref(),
            POOL_AUTHORITY_SEED,
            &[authority_bump],
        ];
        let signer_seeds = &[&authority_seeds[..]];

        anchor_spl::token_interface::transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: input_account.to_account_info(),
                    mint: input_mint.to_account_info(),
                    to: input_pool_account.to_account_info(),
                    authority: self.trader.to_account_info(),
                },
            ),
            input_amount,
            input_mint.decimals,
        )?;

        anchor_spl::token_interface::transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: output_pool_account.to_account_info(),
                    mint: output_mint.to_account_info(),
                    to: output_account.to_account_info(),
                    authority: self.pool_authority.to_account_info(),
                },
                signer_seeds,
            ),
            output,
            output_mint.decimals,
        )?;

        emit!(SwapEvent {
            trader: self.trader.key(),
            swap_a,
            input_amount,
            net_input_amount: taxed_input,
            output_amount: output,
            fee_bps: self.amm.fee,
            temperature: self.sst.temperature,
            weight_end: weights.weight_end,
            weight_gaia: weights.weight_gaia,
            reserve_a: self.pool_account_a.amount,
            reserve_b: self.pool_account_b.amount,
        });

        self.pool_account_a.reload()?;
        self.pool_account_b.reload()?;
        let invariant_after = compute_weighted_invariant(
            self.pool_account_a.amount,
            self.pool_account_b.amount,
            &weights,
        )?;
        // Allow a tiny epsilon for float math; invariant should not decrease meaningfully.
        require!(
            invariant_after + 1e-9 >= invariant,
            AmmError::InvariantViolated
        );

        Ok(())
    }
}
