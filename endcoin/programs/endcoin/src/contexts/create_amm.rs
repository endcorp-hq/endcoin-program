use crate::{constants::AMM_SEED, errors::*, state::Amm};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(fee: u16)]
pub struct CreateAmm<'info> {
    // The AMM account
    #[account(
        init,
        payer = authority,
        space = Amm::LEN,
        seeds = [
            AMM_SEED,
        ],
        bump,
        constraint = fee >= 5 && fee < 10000 @ AmmError::InvalidFee,
    )]
    pub amm: Box<Account<'info, Amm>>,

    /// The admin of the AMM
    #[account(
        constraint = admin.is_signer @ AmmError::UnauthorizedAdmin
    )]
    pub admin: Signer<'info>,

    // The account paying for all rents
    #[account(mut)]
    pub authority: Signer<'info>,
    // Solana ecosystem accounts
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(new_admin: Pubkey)]
pub struct UpdateAmm<'info> {
    #[account(
        mut,
        seeds = [
            AMM_SEED
        ],
        bump,
    )]
    pub amm: Account<'info, Amm>,

    #[account(
        constraint = admin.is_signer @ AmmError::UnauthorizedAdmin
    )]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(new_fee: u16)]
pub struct UpdateFee<'info> {
    #[account(
        mut,
        seeds = [
            AMM_SEED
        ],
        bump,
    )]
    pub amm: Account<'info, Amm>,

    #[account(
        constraint = admin.is_signer @ AmmError::UnauthorizedAdmin
    )]
    pub admin: Signer<'info>,
}

impl<'info> CreateAmm<'info> {
    pub fn create_amm(&mut self, fee: u16) -> Result<()> {
        // Check if the AMM has already been created
        match self.amm.created {
            true => return Err(AmmError::AlreadyCreated.into()),
            false => {
                // set inner values of amm
                self.amm.set_inner(Amm {
                    admin: self.admin.key(),
                    fee,
                    created: true,
                });
                Ok(())
            }
        }
    }
}

impl<'info> UpdateAmm<'info> {
    pub fn update_admin(&mut self, new_admin: Pubkey) -> Result<()> {
        // Check if the AMM has already been created
        require!(self.amm.created, AmmError::NotCreated);

        // Add in a check for the admin's signature
        match self.admin.key() == self.amm.admin {
            true => {
                self.amm.admin = new_admin;
                msg!("Admin Updated");
                return Ok(());
            }
            false => return Err(AmmError::NotSigner.into()),
        }
    }
}

impl<'info> UpdateFee<'info> {
    pub fn update_fee(&mut self, new_fee: u16) -> Result<()> {
        // Check if the AMM has already been created
        require!(self.amm.created, AmmError::NotCreated);

        // Add in a check for the admin's signature
        match self.admin.key() == self.amm.admin {
            true => {
                self.amm.fee = new_fee;
                msg!("Fee Updated");
                return Ok(());
            }
            false => return Err(AmmError::NotSigner.into()),
        }
    }
}
