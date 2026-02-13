use anchor_lang::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use switchboard_on_demand::on_demand::accounts::pull_feed::PullFeedAccountData;

use crate::{
    constants::{AMM_SEED, MAX_ORACLE_STALENESS_SECONDS, SST_SEED},
    errors::AmmError,
    events::SstUpdatedEvent,
    math::temperature_weights,
    state::{Amm, SST},
};

#[derive(Accounts)]
pub struct PullFeed<'info> {
    #[account(seeds = [AMM_SEED], bump)]
    pub amm: Box<Account<'info, Amm>>,

    #[account(
        mut,
        seeds = [SST_SEED, amm.key().as_ref()],
        bump,
        constraint = sst.created @ AmmError::InvalidTemperature,
    )]
    pub sst: Box<Account<'info, SST>>,

    /// CHECK: validated by key match + Switchboard account parsing.
    pub feed: AccountInfo<'info>,
}

impl<'info> PullFeed<'info> {
    pub fn pull_feed(&mut self) -> Result<()> {
        require_keys_eq!(
            self.feed.key(),
            self.sst.oracle_feed,
            AmmError::OracleFeedMismatch
        );

        let clock = Clock::get()?;
        let feed_account = self.feed.data.borrow();
        let feed =
            PullFeedAccountData::parse(feed_account).map_err(|_| AmmError::InvalidOracleFeed)?;

        let value = feed
            .value(clock.slot)
            .map_err(|_| AmmError::OracleFeedStale)?;
        let temperature = value.to_f64().ok_or(AmmError::InvalidOracleFeed)?;
        let staleness = clock
            .unix_timestamp
            .checked_sub(feed.last_update_timestamp)
            .ok_or(AmmError::OracleFeedStale)?;
        require!(
            staleness <= MAX_ORACLE_STALENESS_SECONDS,
            AmmError::OracleFeedStale
        );
        temperature_weights(temperature)?;

        self.sst.temperature = temperature;
        self.sst.last_updated_slot = clock.slot;
        self.sst.last_updated_unix_timestamp = clock.unix_timestamp;

        emit!(SstUpdatedEvent {
            feed: self.feed.key(),
            temperature,
            slot: clock.slot,
            unix_timestamp: clock.unix_timestamp,
        });
        Ok(())
    }
}
