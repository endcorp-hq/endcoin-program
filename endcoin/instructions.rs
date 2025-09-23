use anchor_lang::{prelude::*, solana_program::entrypoint::ProgramResult};

use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::spl_token_2022::extension::{
        group_member_pointer::GroupMemberPointer, metadata_pointer::MetadataPointer,
        mint_close_authority::MintCloseAuthority, permanent_delegate::PermanentDelegate,
        transfer_hook::TransferHook,
    },
    token::{mint_to, MintTo},
    token_interface::{
        spl_token_metadata_interface::state::TokenMetadata, token_metadata_initialize, Mint,
        Token2022, TokenAccount, TokenMetadataInitialize,
    },
};

use spl_pod::optional_keys::OptionalNonZeroPubkey;

use crate::{
    constants::{AMM_SEED, AUTHORITY_SEED, META_LIST_ACCOUNT_SEED, POOL_AUTHORITY_SEED, SST_SEED}, get_meta_list_size,  get_mint_extension_data, update_account_lamports_to_minimum_balance, AmmError, SstError, SST
};
use crate::state::{Pool, Amm};

#[derive(AnchorDeserialize, AnchorSerialize)]
pub struct InitializeArgs {
    pub token: bool,
}

#[derive(Accounts)]
#[instruction(args: InitializeArgs, id: Pubkey, fee: u16)]
pub struct Initialize<'info> {

    // SST Account
    #[account(
        init,
        payer = payer,
        space = SST::LEN,
        seeds = [
            SST_SEED,
        ],
        bump
    )] pub sst: Box<Account<'info, SST>>,

    // The AMM account
    #[account(
        init_if_needed,
        payer = payer,
        space = Amm::LEN,
        seeds = [
            AMM_SEED,
        ],
        bump,
        constraint = fee < 10000 @ AmmError::InvalidFee,
    )]
    pub amm: Box<Account<'info, Amm>>,

    /// The admin of the AMM
    /// CHECK: Read only, delegatable creation
    pub admin: AccountInfo<'info>,

    // Endcoin Mint
    #[account(
        init,
        signer,
        payer = payer,
        mint::token_program = token_program,
        mint::decimals = 6,
        mint::authority = authority,
        mint::freeze_authority = authority,
        extensions::metadata_pointer::authority = authority,
        extensions::metadata_pointer::metadata_address = endcoin,
        extensions::group_member_pointer::authority = authority,
        extensions::group_member_pointer::member_address = endcoin,
        extensions::transfer_hook::authority = authority,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::close_authority::authority = authority,
        extensions::permanent_delegate::delegate = authority,
        
    )]
    pub endcoin: Box<InterfaceAccount<'info, Mint>>,

    // Gaiacoin Mint
    #[account(
        init,
        signer,
        payer = payer,
        mint::token_program = token_program,
        mint::decimals = 6,
        mint::authority = authority,
        mint::freeze_authority = authority,
        extensions::metadata_pointer::authority = authority,
        extensions::metadata_pointer::metadata_address = gaiacoin,
        extensions::group_member_pointer::authority = authority,
        extensions::group_member_pointer::member_address = gaiacoin,
        extensions::transfer_hook::authority = authority,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::close_authority::authority = authority,
        extensions::permanent_delegate::delegate = authority,
    )]
    pub gaiacoin: Box<InterfaceAccount<'info, Mint>>,

    // extra metas account
    /// CHECK: This account's data is a buffer of TLV data
    #[account(
        init,
        space = get_meta_list_size(None),
        seeds = [META_LIST_ACCOUNT_SEED, endcoin.key().as_ref()],
        bump,
        payer = payer,
    )]
    pub extra_metas_account_endcoin: UncheckedAccount<'info>,
        /// CHECK: This account's data is a buffer of TLV data
        #[account(
            init,
            space = get_meta_list_size(None),
            seeds = [META_LIST_ACCOUNT_SEED, gaiacoin.key().as_ref()],
            bump,
            payer = payer,
        )]
        pub extra_metas_account_gaiacoin: UncheckedAccount<'info>,

    // Mint Authority
    /// CHECK: Read only authority
    #[account(
        seeds = [
            AUTHORITY_SEED.as_ref(),
        ],
        bump,
    )]
    pub authority: AccountInfo<'info>,

    // the pool account
    #[account(
        init,
        payer = payer,
        space = Pool::LEN,
        seeds = [
            amm.key().as_ref(),
            endcoin.key().as_ref(),
            gaiacoin.key().as_ref(),
        ],
        bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    // pool endcoin mint ATA
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = endcoin,
        associated_token::token_program = token_program,
        associated_token::authority = pool,
    )]
    pub pool_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    // pool gaiacoin mint ATA
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = gaiacoin,
        associated_token::token_program = token_program,
        associated_token::authority = pool,
    )]
    pub pool_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    // pulse token mint
    #[account(
        init,
        signer,
        payer = payer,
        mint::token_program = token_program,
        mint::decimals = 6,
        mint::authority = pool_authority,
        mint::freeze_authority = pool_authority,
        extensions::metadata_pointer::authority = pool_authority,
        extensions::metadata_pointer::metadata_address = gaiacoin,
        extensions::group_member_pointer::authority = pool_authority,
        extensions::group_member_pointer::member_address = gaiacoin,
        extensions::transfer_hook::authority = pool_authority,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::close_authority::authority = pool_authority,
        extensions::permanent_delegate::delegate = pool_authority,
    )]
    pub mint_liquidity_pool: Box<InterfaceAccount<'info, Mint>>,

    // the pool authority
    /// CHECK: Read only authority
    #[account(
        seeds = [
            POOL_AUTHORITY_SEED.as_ref(),
        ],
        bump,
    )]
    pub pool_authority: AccountInfo<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}


impl<'info> Initialize<'info> {

    // create the SST Struct, and fill it with a default value of 21 degrees
    pub fn create_sst(
        &mut self
    ) -> Result<()> {
        let sst = &mut self.sst;
        if sst.created == true {

            return Err(SstError::AlreadyInitialized.into());
        
        } else {
        
            sst.temperature = 21.000;
            sst.created = true;
        
        }
        
        Ok(())

    }

    pub fn create_amm(
        &mut self, 
        id: Pubkey, 
        fee: u16
    ) -> Result<()> {
        if self.amm.created == true {
            
            msg!("AMM Is Locked, Cannot Create");

            return Err(AmmError::AlreadyCreated.into());
        
        } else {

            // set inner values of amm
            self.amm.set_inner(
                Amm {
                    id,
                    admin: self.admin.key(),
                    fee,
                    created: true
                }
            );
            
            msg!("AMM Created, setting State to True");
        
            Ok(())
        
        }
    }

    pub fn initialize_token_metadata(
        &mut self,
        token: bool,
    ) -> ProgramResult {
        
        let name: &str;
        let symbol: &str;
        let uri: &str;

        match token {
            false => {
                name = "Endcoin";
                symbol = "END";
                uri = "https://arweave.net/K-3ZVae8bb_iMFLTZKuO_7CMNLtOc-bmMafhhn_PeF0";
            
            // Set mint to pool
            self.pool.mint_a = self.endcoin.key();

            let cpi_accounts = TokenMetadataInitialize {
                token_program_id: self.token_program.to_account_info(),
                mint: self.endcoin.to_account_info(),
                metadata: self.endcoin.to_account_info(), // metadata account is the mint, since data is stored in mint
                mint_authority: self.authority.to_account_info(),
                update_authority: self.authority.to_account_info(),
            };
            
            let cpi_ctx = CpiContext::new(self.token_program.to_account_info(), cpi_accounts);
            token_metadata_initialize(cpi_ctx, name.to_string(), symbol.to_string(), uri.to_string())?;
            }
            true => {
                name = "Gaiacoin";
                symbol = "GAIA";
                uri = "https://arweave.net/aFr3D_1_O4F2cDpE9r_OGP5o6D2OyZgDzlLmBEdUYos";
                
                // Set mint to pool
                self.pool.mint_b = self.gaiacoin.key();

                let cpi_accounts = TokenMetadataInitialize {
                    token_program_id: self.token_program.to_account_info(),
                    mint: self.gaiacoin.to_account_info(),
                    metadata: self.gaiacoin.to_account_info(), // metadata account is the mint, since data is stored in mint
                    mint_authority: self.authority.to_account_info(),
                    update_authority: self.authority.to_account_info(),
                };
                
                let cpi_ctx = CpiContext::new(self.token_program.to_account_info(), cpi_accounts);
                token_metadata_initialize(cpi_ctx, name.to_string(), symbol.to_string(), uri.to_string())?;
            }
        };
        
    
        Ok(())
    }

    pub fn create_pool(
        &mut self,
        bumps: InitializeBumps
    ) -> Result<()> {

        let initial_amount: u64 = 6920508;
        
        let seeds = &[
            AUTHORITY_SEED,
            &[bumps.authority]
        ];

        let signer_seeds = &[&seeds[..]];


        let _ = mint_to(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(), 
                MintTo
                {
                    mint: self.endcoin.to_account_info(),
                    to: self.pool_account_a.to_account_info(),
                    authority: self.authority.to_account_info(),
                } ,
                signer_seeds
            ),
            initial_amount
        );

        let _ = mint_to(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(), 
                MintTo
                {
                    mint: self.gaiacoin.to_account_info(),
                    to: self.pool_account_b.to_account_info(),
                    authority: self.authority.to_account_info(),
                } ,
                signer_seeds
            ),
            initial_amount
        );
        


        let pool = &mut self.pool;
        pool.amm = self.amm.key();
        pool.mint_a = self.endcoin.key();
        pool.mint_b = self.gaiacoin.key();

        Ok(())

        // mint and deposit liquidity to the depositor/initializor ATA then sent to the pool. 


    }



}

pub fn handler(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
    
    ctx.accounts.initialize_token_metadata(
        args.token
    )?;
    
    // variable to hold whichever mint
    let token_mint;
    // mint A
    let endcoin = &mut ctx.accounts.endcoin;
    // mint B
    let gaiacoin = &mut ctx.accounts.gaiacoin;

    // select which mint to use
    match args.token {
        false => {
            token_mint = endcoin;
        }
        true => {
            token_mint = gaiacoin;
        }
    }

    token_mint.reload()?;
    let mint_data = &mut token_mint.to_account_info();
    let metadata = get_mint_extension_data::<TokenMetadata>(mint_data)?;
    assert_eq!(metadata.mint, token_mint.key());
    let metadata_pointer = get_mint_extension_data::<MetadataPointer>(mint_data)?;
    let mint_key: Option<Pubkey> = Some(token_mint.key());
    let authority_key: Option<Pubkey> = Some(ctx.accounts.authority.key());
    assert_eq!(
        metadata_pointer.metadata_address,
        OptionalNonZeroPubkey::try_from(mint_key)?
    );
    assert_eq!(
        metadata_pointer.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    let permanent_delegate = get_mint_extension_data::<PermanentDelegate>(mint_data)?;
    assert_eq!(
        permanent_delegate.delegate,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    let close_authority = get_mint_extension_data::<MintCloseAuthority>(mint_data)?;
    assert_eq!(
        close_authority.close_authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    let transfer_hook = get_mint_extension_data::<TransferHook>(mint_data)?;
    let program_id: Option<Pubkey> = Some(ctx.program_id.key());
    assert_eq!(
        transfer_hook.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    assert_eq!(
        transfer_hook.program_id,
        OptionalNonZeroPubkey::try_from(program_id)?
    );
    let group_member_pointer = get_mint_extension_data::<GroupMemberPointer>(mint_data)?;
    assert_eq!(
        group_member_pointer.authority,
        OptionalNonZeroPubkey::try_from(authority_key)?
    );
    assert_eq!(
        group_member_pointer.member_address,
        OptionalNonZeroPubkey::try_from(mint_key)?
    );
    update_account_lamports_to_minimum_balance(
        token_mint.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.system_program.to_account_info(),
    )?;

    Ok(())
}









#[derive(Accounts)]
#[instruction()]
pub struct CheckMintExtensionConstraints<'info> {
    #[account(mut)]
    /// CHECK: can be any account
    pub authority: Signer<'info>,
    #[account(
        extensions::metadata_pointer::authority = authority,
        extensions::metadata_pointer::metadata_address = mint,
        extensions::group_member_pointer::authority = authority,
        extensions::group_member_pointer::member_address = mint,
        extensions::transfer_hook::authority = authority,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::close_authority::authority = authority,
        extensions::permanent_delegate::delegate = authority,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
}