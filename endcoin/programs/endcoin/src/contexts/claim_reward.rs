use anchor_lang::prelude::*;

use crate::AmmError;
use crate::{constants::REWARD_VAULT_SEED, state::Pool, RewardVault};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, Token2022, TokenAccount},
};

impl<'info> ClaimReward<'info> {
    pub fn claim_reward(&mut self, claimer: Pubkey, amount_a: u64, amount_b: u64) -> Result<()> {
        // check the payer is signer
        if self.claimer.key != &claimer {
            return Err(AmmError::UnauthorizedClaimer.into());
        }

        // check if the reward account has enough balance
        if self.reward_account_a.amount < amount_a || self.reward_account_b.amount < amount_b {
            return Err(AmmError::InsufficientReward.into());
        }

        let cpi_program_a = self.token_program.to_account_info();

        let pool_key = self.pool.key();
        let mint_a_key = self.mint_a.key();
        let mint_b_key = self.mint_b.key();

        let seeds = [
            pool_key.as_ref(),
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            REWARD_VAULT_SEED,
            &[self.reward_vault.bump],
        ];

        let signer_seeds = &[&seeds[..]];

        // Transfer tokens from taker to initializer
        let cpi_accounts_a = anchor_spl::token_interface::TransferChecked {
            from: self.reward_account_a.to_account_info().clone(),
            mint: self.mint_a.to_account_info().clone(),
            to: self.to_mint_a_account.to_account_info().clone(),
            authority: self.reward_vault.to_account_info().clone(),
        };

        // coconuts to mints
        anchor_spl::token_interface::transfer_checked(
            CpiContext::new_with_signer(cpi_program_a, cpi_accounts_a, signer_seeds),
            amount_a,
            6,
        )?;

        // Transfer tokens from taker to initializer
        let cpi_accounts_b = anchor_spl::token_interface::TransferChecked {
            from: self.reward_account_b.to_account_info().clone(),
            mint: self.mint_b.to_account_info().clone(),
            to: self.to_mint_b_account.to_account_info().clone(),
            authority: self.reward_vault.to_account_info().clone(),
        };

        let cpi_program_b = self.token_program.to_account_info();

        anchor_spl::token_interface::transfer_checked(
            CpiContext::new_with_signer(cpi_program_b, cpi_accounts_b, signer_seeds),
            amount_b,
            6,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct ClaimReward<'info> {
    #[account(
        seeds = [
            pool.amm.as_ref(),
            pool.mint_a.key().as_ref(),
            pool.mint_b.key().as_ref(),
        ],
        bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(mut)]
    claimer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint_a,
        associated_token::authority = claimer
    )]
    pub to_mint_a_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint_b,
        associated_token::authority = claimer
    )]
    pub to_mint_b_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        seeds = [
            pool.key().as_ref(),
            mint_a.key().as_ref(),
            mint_b.key().as_ref(),
            REWARD_VAULT_SEED,
        ],
        bump = reward_vault.bump,
    )]
    pub reward_vault: Box<Account<'info, RewardVault>>,

    #[account(
        mut,
        constraint = reward_account_a.owner == reward_vault.key(),
        constraint = reward_account_a.mint == mint_a.key(),
    )]
    pub reward_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = reward_account_b.owner == reward_vault.key(),
        constraint = reward_account_b.mint == mint_b.key(),
    )]
    pub reward_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Solana ecosystem accounts
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token2022>,
}
