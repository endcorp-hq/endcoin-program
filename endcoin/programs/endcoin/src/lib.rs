use anchor_lang::prelude::*;

pub use errors::*;
pub mod errors;
pub use state::*;
pub mod state;
mod constants;

pub use contexts::*;
pub mod contexts;


declare_id!("B6tCcVMPoQYtznwRNLAoKnwMQy95c73jg8AQBp3v9dY2");

#[program]
pub mod endcoin {
    use super::*;

    pub fn create_amm(ctx: Context<CreateAmm>, fee: u16) -> Result<()> {
        // Validate fee is within acceptable range (0-100%)
        require!(fee <= 10000, AmmError::InvalidFee); // 10000 = 100.00%
        ctx.accounts.create_amm(fee)?;
        Ok(())
    }

    pub fn update_admin(ctx: Context<UpdateAmm>, new_admin: Pubkey) -> Result<()> {
        ctx.accounts.update_admin(new_admin)?;
        Ok(())
    }
    pub fn update_fee(ctx: Context<UpdateFee>, new_fee: u16) -> Result<()> {
        ctx.accounts.update_fee(new_fee)?;
        Ok(())
    }
    pub fn create_sst(ctx: Context<CreateSST>) -> Result<()> {
        ctx.accounts.create_sst()?;
        Ok(())
    }

    pub fn create_pool(
        ctx: Context<CreatePool>
    ) -> Result<()> {
        ctx.accounts.create_pool()?;
        Ok(())
    }
    pub fn create_token_accounts(
        ctx: Context<CreateTokenAccounts>
    ) -> Result<()> {
        ctx.accounts.create_token_accounts()?;
        Ok(())
    }
    pub fn create_reward_vault(
        ctx: Context<CreateRewardVault>
    ) -> Result<()> {
        ctx.accounts.create_reward_vault(&ctx.bumps)?;
        Ok(())
    }
    pub fn create_reward_token_accounts(
        ctx: Context<CreateRewardTokenAccounts>
    ) -> Result<()> {
        ctx.accounts.create_reward_token_accounts()?;
        Ok(())
    }

    // pub fn create_endcoin(ctx: Context<CreateEndcoin>) -> Result<()>
    // {
    //     ctx.accounts.initialize_endcoin()?;
    //     Ok(())
    // }
    // pub fn create_gaiacoin(ctx: Context<CreateGaiacoin>) -> Result<()>
    // {
    //     ctx.accounts.initialize_gaiacoin()?;
    //     Ok(())
    // }

    pub fn deposit_liquidity(
        ctx: Context<DepositLiquidity>, mean_temp: f64
    ) -> Result<()> {
        ctx.accounts.deposit_liquidity(&ctx.bumps, mean_temp)
    }

    pub fn deposit_rewards(
        ctx: Context<DepositRewards>,
        mean_temp: f64
    ) -> Result<()> {
        ctx.accounts.deposit_rewards(&ctx.bumps, mean_temp)
    }

    pub fn claim_reward(
        ctx: Context<ClaimReward>,
        claimer: Pubkey, amount_a: u64, amount_b: u64
    ) -> Result<()> {
        ctx.accounts.claim_reward(claimer, amount_a, amount_b)
    }
    
    pub fn swap_exact_tokens_for_tokens(
        ctx: Context<SwapExactTokensForTokens>,
        swap_a: bool,
        input_amount: u64,
        min_output_amount: u64,
    ) -> Result<()> {
        ctx.accounts.swap_exact_tokens_for_tokens(swap_a, input_amount, min_output_amount, &ctx.bumps)
    }

    pub fn pull_feed(ctx: Context<PullFeed>) -> Result<()> {
        ctx.accounts.pull_feed()?;
        Ok(())
    }

    pub fn update_timestamp(ctx: Context<TimeState>) -> Result<()> {
        ctx.accounts.update_timestamp()?;
        Ok(())
    }

    // pub fn create_mint_account(
    //     ctx: Context<CreateMintAccount>,
    //     args: CreateMintAccountArgs,
    // ) -> Result<()> {
    //     instructions::handler(ctx, args)
    // }

    // pub fn check_mint_extensions_constraints(
    //     _ctx: Context<CheckMintExtensionConstraints>,
    // ) -> Result<()> {
    //     Ok(())
    // }







}
