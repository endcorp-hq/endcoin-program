use anchor_lang::prelude::*;

use crate::{
    constants::{AMM_SEED, POOL_AUTHORITY_SEED},
    errors::*,
    state::{Amm, Pool},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, Token2022, TokenAccount},
};

impl<'info> CreatePool<'info> {
    pub fn create_pool(&mut self) -> Result<()> {
        let reserve_a = anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &self.pool_authority.key(),
            &self.mint_a.key(),
            &self.token_program.key(),
        );
        let reserve_b = anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &self.pool_authority.key(),
            &self.mint_b.key(),
            &self.token_program.key(),
        );
        let pool = &mut self.pool;
        pool.amm = self.amm.key();
        pool.mint_a = self.mint_a.key();
        pool.mint_b = self.mint_b.key();
        pool.reserve_a = reserve_a;
        pool.reserve_b = reserve_b;

        Ok(())
    }
}

impl<'info> CreateTokenAccounts<'info> {
    pub fn create_token_accounts(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(
        seeds = [
            AMM_SEED
        ],
        bump,
    )]
    pub amm: Box<Account<'info, Amm>>,

    #[account(
        init,
        payer = payer,
        space = Pool::LEN,
        seeds = [
            amm.key().as_ref(),
            mint_a.key().as_ref(),
            mint_b.key().as_ref(),
        ],
        bump,
        constraint = mint_a.key() != mint_b.key() @ AmmError::InvalidMint,
    )]
    pub pool: Box<Account<'info, Pool>>,
    /// CHECK: Read only authority
    #[account(
        seeds = [
            amm.key().as_ref(),
            mint_a.key().as_ref(),
            mint_b.key().as_ref(),
            POOL_AUTHORITY_SEED,
        ],
        bump,
    )]
    pub pool_authority: AccountInfo<'info>,

    pub mint_liquidity: Box<InterfaceAccount<'info, Mint>>,

    pub mint_a: Box<InterfaceAccount<'info, Mint>>,

    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    /// The account paying for all rents
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Solana ecosystem accounts
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct CreateTokenAccounts<'info> {
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
        init,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = pool_authority,
        associated_token::token_program = token_program,
        constraint = pool_account_a.key() == pool.reserve_a @ AmmError::InvalidPoolAccount,
    )]
    pub pool_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = pool_authority,
        associated_token::token_program = token_program,
        constraint = pool_account_b.key() == pool.reserve_b @ AmmError::InvalidPoolAccount,
    )]
    pub pool_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Read only authority
    #[account(
        seeds = [
            amm.key().as_ref(),
            mint_a.key().as_ref(),
            mint_b.key().as_ref(),
            POOL_AUTHORITY_SEED,
        ],
        bump,
    )]
    pub pool_authority: AccountInfo<'info>,

    #[account(seeds = [AMM_SEED], bump)]
    pub amm: Box<Account<'info, Amm>>,

    pub mint_a: Box<InterfaceAccount<'info, Mint>>,

    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    /// The account paying for all rents
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token2022>,
}
