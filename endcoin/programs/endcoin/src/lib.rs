use anchor_lang::prelude::*;

pub use errors::*;
pub mod errors;
pub use state::*;
mod constants;
mod events;
mod math;
pub mod state;
pub use events::*;

pub use contexts::*;
pub mod contexts;

declare_id!("Fyg2zFo8HzsyHqeNE2DRabhHZCekwY42jTVjivTfT8HB");

#[program]
pub mod endcoin {
    use super::*;

    pub fn create_amm(ctx: Context<CreateAmm>, fee: u16) -> Result<()> {
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
    pub fn create_sst(ctx: Context<CreateSST>, oracle_feed: Pubkey) -> Result<()> {
        ctx.accounts.create_sst(oracle_feed)?;
        Ok(())
    }

    pub fn create_pool(ctx: Context<CreatePool>) -> Result<()> {
        ctx.accounts.create_pool()?;
        Ok(())
    }
    pub fn create_token_accounts(ctx: Context<CreateTokenAccounts>) -> Result<()> {
        ctx.accounts.create_token_accounts()?;
        Ok(())
    }
    pub fn create_reward_vault(
        ctx: Context<CreateRewardVault>,
        whitelist_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .create_reward_vault(&ctx.bumps, whitelist_authority)?;
        Ok(())
    }

    pub fn update_reward_whitelist(
        ctx: Context<UpdateRewardWhitelist>,
        whitelist_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts.update_reward_whitelist(whitelist_authority)
    }
    pub fn create_reward_token_accounts(ctx: Context<CreateRewardTokenAccounts>) -> Result<()> {
        ctx.accounts.create_reward_token_accounts()?;
        Ok(())
    }

    pub fn deposit_liquidity(ctx: Context<DepositLiquidity>) -> Result<()> {
        ctx.accounts.deposit_liquidity(&ctx.bumps)
    }

    pub fn deposit_rewards(ctx: Context<DepositRewards>) -> Result<()> {
        ctx.accounts.deposit_rewards(&ctx.bumps)
    }

    pub fn claim_reward(ctx: Context<ClaimReward>, amount_a: u64, amount_b: u64) -> Result<()> {
        ctx.accounts.claim_reward(amount_a, amount_b)
    }

    pub fn swap(
        ctx: Context<SwapExactTokensForTokens>,
        swap_a: bool,
        input_amount: u64,
        min_output_amount: u64,
    ) -> Result<()> {
        ctx.accounts
            .swap(swap_a, input_amount, min_output_amount, &ctx.bumps)
    }

    pub fn pull_feed(ctx: Context<PullFeed>) -> Result<()> {
        ctx.accounts.pull_feed()?;
        Ok(())
    }

    pub fn update_timestamp(ctx: Context<TimeState>) -> Result<()> {
        ctx.accounts.update_timestamp()?;
        Ok(())
    }
}
