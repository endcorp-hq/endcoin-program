use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to, Mint, MintTo, Token2022, TokenAccount},
};

use crate::{
    constants::{POOL_AUTHORITY_SEED, REWARD_VAULT_SEED},
    errors::AmmError,
    math::{calculate_emissions, EmissionTarget},
    Pool, RewardVault,
};

impl<'info> DepositLiquidity<'info> {
    pub fn deposit_liquidity(
        &mut self,
        bumps: &DepositLiquidityBumps,
        mean_temp: f64,
    ) -> Result<()> {
        // compute result
        let emissions = calculate_emissions(mean_temp, EmissionTarget::Pool)?;
        let amount_a = emissions.amount_a;
        let amount_b = emissions.amount_b;
        let liquidity = emissions.liquidity;

        require!(
            amount_a > 0 && amount_b > 0 && liquidity > 0,
            AmmError::DepositTooSmall
        );

        // Mint tokens directly to pool, as we don't need a depositor.
        let seeds = &["authority".as_bytes(), &[bumps.mint_authority]];
        let mint_signer_seeds = &[&seeds[..]];

        // minting the correct amount of tokens to the pool for token a
        self.mint_token(
            self.mint_a.to_account_info(),
            self.pool_account_a.to_account_info(),
            self.mint_authority.to_account_info(),
            amount_a,
            mint_signer_seeds,
            self.token_program.to_account_info(),
        )?;

        // minting the correct amount of tokens to the pool for token b
        self.mint_token(
            self.mint_b.to_account_info(),
            self.pool_account_b.to_account_info(),
            self.mint_authority.to_account_info(),
            amount_b,
            mint_signer_seeds,
            self.token_program.to_account_info(),
        )?;

        // Mint the liquidity token to the AMM.
        let authority_bump = bumps.pool_authority;
        let authority_seeds = &[
            &self.pool.amm.key().to_bytes(),
            &self.mint_a.key().to_bytes(),
            &self.mint_b.key().to_bytes(),
            b"pool-authority".as_ref(),
            &[authority_bump],
        ];
        let liquidity_signer_seeds = &[&authority_seeds[..]];

        self.mint_token(
            self.mint_liquidity.to_account_info(),
            self.depositor_account_liquidity.to_account_info(),
            self.pool_authority.to_account_info(),
            liquidity,
            liquidity_signer_seeds,
            self.token_program.to_account_info(),
        )?;

        Ok(())
    }

    pub fn mint_token(
        &mut self,
        mint: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        amount: u64,
        signer_seeds: &[&[&[u8]]; 1],
        token_program: AccountInfo<'info>,
    ) -> Result<()> {
        mint_to(
            CpiContext::new_with_signer(
                token_program,
                MintTo {
                    mint,
                    to,
                    authority,
                },
                signer_seeds,
            ),
            amount,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct DepositLiquidity<'info> {
    #[account(
        seeds = [
            pool.amm.as_ref(),
            pool.mint_a.key().as_ref(),
            pool.mint_b.key().as_ref(),
        ],
        bump,
        has_one = mint_a,
        has_one = mint_b,
    )]
    pub pool: Box<Account<'info, Pool>>,

    ///CHECK: The Pool authority account
    #[account(
        mut,
        seeds = [
            pool.amm.key().as_ref(),
            pool.mint_a.key().as_ref(),
            pool.mint_b.key().as_ref(),
            POOL_AUTHORITY_SEED,
        ],
        bump,
    )]
    pub pool_authority: AccountInfo<'info>,

    /// The account paying for all rents
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub mint_liquidity: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub pool_account_a: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub pool_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_liquidity,
        associated_token::authority = pool_authority,
    )]
    pub depositor_account_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Solana ecosystem accounts
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// CHECK: Mint authority account
    #[account(
        mut,
        seeds = [
            b"authority"
            ],
        bump
    )]
    pub mint_authority: UncheckedAccount<'info>,
}
#[derive(Accounts)]
pub struct DepositRewards<'info> {
    #[account(
        seeds = [
            pool.key().as_ref(),
            mint_a.key().as_ref(),
            mint_b.key().as_ref(),
            REWARD_VAULT_SEED,
        ],
        bump,
        has_one = mint_a,
        has_one = mint_b,
    )]
    pub reward_vault: Box<Account<'info, RewardVault>>,

    #[account(
        seeds = [
            pool.amm.as_ref(),
            pool.mint_a.key().as_ref(),
            pool.mint_b.key().as_ref(),
        ],
        bump,
        has_one = mint_a,
        has_one = mint_b,
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// The account paying for all rents
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(

        init_if_needed,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = reward_vault,
    )]
    pub reward_account_a: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = reward_vault,
    )]
    pub reward_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Solana ecosystem accounts
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// CHECK: Mint authority account
    #[account(
        mut,
        seeds = [
            b"authority"
            ],
        bump
    )]
    pub mint_authority: UncheckedAccount<'info>,
}

impl<'info> DepositRewards<'info> {
    pub fn deposit_rewards(&mut self, bumps: &DepositRewardsBumps, mean_temp: f64) -> Result<()> {
        // compute result
        let emissions = calculate_emissions(mean_temp, EmissionTarget::Rewards)?;

        let amount_a = emissions.amount_a;
        let amount_b = emissions.amount_b;

        require!(amount_a > 0 && amount_b > 0, AmmError::DepositTooSmall);

        // Mint tokens directly to reward vault, as we don't need a depositor.
        let seeds = &["authority".as_bytes(), &[bumps.mint_authority]];
        let mint_signer_seeds = &[&seeds[..]];

        // minting the correct amount of tokens to the reward vault for token a
        self.mint_token(
            self.mint_a.to_account_info(),
            self.reward_account_a.to_account_info(),
            self.mint_authority.to_account_info(),
            amount_a,
            mint_signer_seeds,
            self.token_program.to_account_info(),
        )?;

        // minting the correct amount of tokens to the pool for token b
        self.mint_token(
            self.mint_b.to_account_info(),
            self.reward_account_b.to_account_info(),
            self.mint_authority.to_account_info(),
            amount_b,
            mint_signer_seeds,
            self.token_program.to_account_info(),
        )?;

        Ok(())
    }

    pub fn mint_token(
        &mut self,
        mint: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        amount: u64,
        signer_seeds: &[&[&[u8]]; 1],
        token_program: AccountInfo<'info>,
    ) -> Result<()> {
        mint_to(
            CpiContext::new_with_signer(
                token_program,
                MintTo {
                    mint,
                    to,
                    authority,
                },
                signer_seeds,
            ),
            amount,
        )?;

        Ok(())
    }
}
