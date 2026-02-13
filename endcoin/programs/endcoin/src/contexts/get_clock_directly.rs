use anchor_lang::prelude::*;

#[account]
#[derive(Default, Debug)]
pub struct Time {
    pub created: bool,
    pub timestamp: i64,
}
impl Time {
    pub const LEN: usize = 8 + 8 + 1;
}

#[derive(Accounts, Debug)]
pub struct TimeState<'info> {
    // Time Account
    #[account(
        init_if_needed,
        payer = payer,
        space = Time::LEN,
        seeds = [
            b"time",
        ],
        bump
    )]
    pub time: Account<'info, Time>,

    #[account(mut)]
    pub payer: Signer<'info>,

    // System Program
    pub system_program: Program<'info, System>,
}

// Accounts required
/// 1. [signer, writable] Payer
/// 2. [writable] Hello state account
///
///
///
impl<'info> TimeState<'info> {
    pub fn update_timestamp(&mut self) -> Result<()> {
        // Getting clock directly
        let clock = Clock::get()?;
        // offset equates to 24 hours approx based on solanas 400ms block time
        let offset = 216000;
        // Getting timestamp
        let current_timestamp = clock.unix_timestamp;

        // check if account is initialized
        if self.time.created {
            msg!("Account already initialized, checking timestamp");
            // check if timestamp is greater than current timestamp
            if current_timestamp > (self.time.timestamp + offset) {
                msg!("Timestamp is greater than current timestamp, we're gonna rip.");
                msg!("Allows update");
            } else {
                msg!("No update allowed, not enough time has passed.");
            }
        } else {
            self.time.timestamp = current_timestamp;
            self.time.created = true;
        };

        Ok(())
    }
}
