use anchor_lang::prelude::*;

use crate::{constants::AMM_SEED, state::Amm};
use crate::{constants::SST_SEED, SstError, SST};

#[derive(Accounts)]
#[instruction(oracle_feed: Pubkey)]
pub struct CreateSST<'info> {
    #[account(
        seeds = [
            AMM_SEED
        ],
        bump,
    )]
    pub amm: Account<'info, Amm>,

    // SST Account
    #[account(
        init,
        payer = payer,
        space = SST::LEN,
        seeds = [
            SST_SEED,
            amm.key().as_ref()
        ],
        bump
    )]
    pub sst: Account<'info, SST>,

    /// Only AMM admin can initialize the SST oracle config.
    #[account(mut, address = amm.admin)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // System Program
    pub system_program: Program<'info, System>,
}

// IMPL

impl<'info> CreateSST<'info> {
    // create the SST Struct, and fill it with a default value of 21 degrees
    pub fn create_sst(&mut self, oracle_feed: Pubkey) -> Result<()> {
        let sst = &mut self.sst;
        if sst.created {
            return Err(SstError::AlreadyInitialized.into());
        } else {
            sst.temperature = 21.000;
            sst.created = true;
            sst.oracle_feed = oracle_feed;
            sst.last_updated_slot = 0;
            sst.last_updated_unix_timestamp = 0;
        }

        Ok(())
    }
}
