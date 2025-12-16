use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::TransferChecked,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::{AMM_SEED, POOL_AUTHORITY_SEED},
    errors::*,
    math::{
        apply_fee, compute_weighted_invariant, compute_weighted_swap_output, temperature_weights,
    },
    state::{Amm, Pool, SST},
};

#[derive(Accounts)]
pub struct SwapExactTokensForTokens<'info> {
    #[account(
        seeds = [
            AMM_SEED
        ],
        bump,
    )]
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
        constraint = pool_account_a.owner == pool_authority.key(),
        constraint = pool_account_a.mint == mint_a.key(),
    )]
    pub pool_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = pool_account_b.owner == pool_authority.key(),
        constraint = pool_account_b.mint == mint_b.key(),
    )]
    pub pool_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(constraint = sst.created @ AmmError::InvalidTemperature)]
    pub sst: Box<Account<'info, SST>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = trader,
    )]
    pub trader_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = trader,
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

        // Temperature-aware weights tilt pricing based on current SST.
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

        msg!(
            "Swap {}: input {}, net {}, output {}",
            if swap_a { "A->B" } else { "B->A" },
            input_amount,
            taxed_input,
            output
        );

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
