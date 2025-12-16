use std::str::FromStr;

// Switchboard import
// use switchboard_on_demand::on_demand::accounts::pull_feed::PullFeedAccountData;
use anchor_lang::prelude::{AccountInfo, Error, Pubkey};
use anchor_lang::Accounts;
// Include the feed account
#[derive(Accounts)]
pub struct PullFeed<'info> {
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(
        constraint = feed.key == &Pubkey::from_str("GhCs7zhha7kTyt8EiaWaBT5DREt23GnoPnqa7AU4yv1y").unwrap()
    )]
    pub feed: AccountInfo<'info>,
}

impl<'info> PullFeed<'info> {
    pub fn pull_feed(&mut self) -> Result<(), Error> {
        // // Feed account data
        // let feed_account = self.feed.data.borrow();
        // // Compare the public keys
        // // if ctx.accounts.feed.key != &specific_pubkey {
        // //     throwSomeError
        // // }

        // // Docs at: https://switchboard-on-demand-rust-docs.web.app/on_demand/accounts/pull_feed/struct.PullFeedAccountData.html
        // // let feed = PullFeedAccountData::parse(feed_account).unwrap();

        // let feed = PullFeedAccountData::parse(feed_account).map_err(|_| Error::from(FeedData::InvalidFeedData))?;
        // // Log the value
        // msg!("SST: {:?}", feed.result.value());
        Ok(())
    }
}
