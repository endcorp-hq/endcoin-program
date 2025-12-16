use anchor_lang::prelude::*;

use crate::{constants::SST_SEED, SstError, SST};

#[derive(Accounts)]
pub struct CreateSST<'info> {
    // SST Account
    #[account(
        init,
        payer = payer,
        space = SST::LEN,
        seeds = [
            SST_SEED,
            payer.key().as_ref()
        ],
        bump
    )]
    pub sst: Account<'info, SST>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // System Program
    pub system_program: Program<'info, System>,
}

// IMPL

impl<'info> CreateSST<'info> {
    // create the SST Struct, and fill it with a default value of 21 degrees
    pub fn create_sst(&mut self) -> Result<()> {
        let sst = &mut self.sst;
        if sst.created {
            return Err(SstError::AlreadyInitialized.into());
        } else {
            sst.temperature = 21.000;
            sst.created = true;
        }

        Ok(())
    }
}
