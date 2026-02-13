use anchor_lang::prelude::*;

use crate::{
    constants::{AMM_SEED, SST_SEED},
    errors::{AmmError, SstError},
    events::SstUpdatedEvent,
    math::temperature_weights,
    state::{Amm, SST},
};

#[derive(Accounts)]
pub struct UpdateSstTemperature<'info> {
    #[account(
        seeds = [AMM_SEED],
        bump,
    )]
    pub amm: Box<Account<'info, Amm>>,

    /// Only the AMM admin can update the temperature reading.
    #[account(mut, address = amm.admin @ AmmError::UnauthorizedAdmin)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [
            SST_SEED,
            amm.key().as_ref()
        ],
        bump,
    )]
    pub sst: Box<Account<'info, SST>>,
}

impl<'info> UpdateSstTemperature<'info> {
    pub fn update_sst_temperature(&mut self, temperature: f64) -> Result<()> {
        if !self.sst.created {
            return Err(SstError::NotInitialized.into());
        }

        // Reuse existing validation logic.
        temperature_weights(temperature)?;

        self.sst.temperature = temperature;
        emit!(SstUpdatedEvent {
            admin: self.admin.key(),
            temperature,
        });
        Ok(())
    }
}
