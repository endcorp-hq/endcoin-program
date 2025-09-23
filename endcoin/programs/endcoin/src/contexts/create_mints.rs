use anchor_lang::{prelude::*, solana_program::entrypoint::ProgramResult};

use anchor_spl::{
    associated_token::AssociatedToken,
    // token_2022::spl_token_2022::extension::{
    //     group_member_pointer::GroupMemberPointer, metadata_pointer::MetadataPointer,
    //     mint_close_authority::MintCloseAuthority, permanent_delegate::PermanentDelegate,
    //     transfer_hook::TransferHook,
    // },
    token_interface::{
        // spl_token_metadata_interface::state::TokenMetadata,
        token_metadata_initialize, Mint,
        Token2022, TokenMetadataInitialize,
    },
};

use crate::{
    constants::{AUTHORITY_SEED, META_LIST_ACCOUNT_SEED}, get_meta_list_size,
   
};

// use spl_pod::optional_keys::OptionalNonZeroPubkey;

#[derive(Accounts)]
pub struct CreateEndcoin<'info> {

// Endcoin Mint
#[account(
    init,
    signer,
    payer = payer,
    mint::token_program = token_program,
    mint::decimals = 6,
    mint::authority = authority,
    // mint::freeze_authority = authority,
    // extensions::metadata_pointer::authority = authority,
    // extensions::metadata_pointer::metadata_address = mint_a,
    // extensions::group_member_pointer::authority = authority,
    // extensions::group_member_pointer::member_address = mint_a,
    // extensions::transfer_hook::authority = authority,
    // extensions::transfer_hook::program_id = crate::ID,
    // extensions::close_authority::authority = authority,
    // extensions::permanent_delegate::delegate = authority,
    
)]
pub mint_a: Box<InterfaceAccount<'info, Mint>>,

// extra metas account
/// CHECK: This account's data is a buffer of TLV data
#[account(
    init,
    space = get_meta_list_size(None),
    seeds = [META_LIST_ACCOUNT_SEED, mint_a.key().as_ref()],
    bump,
    payer = payer,
)]
pub extra_metas_account_mint_a: UncheckedAccount<'info>,

// Mint Authority
/// CHECK: Read only authority
#[account(
    seeds = [
        AUTHORITY_SEED.as_ref(),
    ],
    bump,
)]
pub authority: AccountInfo<'info>,

#[account(mut)]
pub payer: Signer<'info>,
pub token_program: Program<'info, Token2022>,
pub associated_token_program: Program<'info, AssociatedToken>,
pub system_program: Program<'info, System>,

}

#[derive(Accounts)]
pub struct CreateGaiacoin<'info> {
// Gaiacoin Mint
#[account(
    init,
    signer,
    payer = payer,
    mint::token_program = token_program,
    mint::decimals = 6,
    mint::authority = authority,
    // mint::freeze_authority = authority,
    // extensions::metadata_pointer::authority = authority,
    // extensions::metadata_pointer::metadata_address = mint_b,
    // extensions::group_member_pointer::authority = authority,
    // extensions::group_member_pointer::member_address = mint_b,
    // extensions::transfer_hook::authority = authority,
    // extensions::transfer_hook::program_id = crate::ID,
    // extensions::close_authority::authority = authority,
    // extensions::permanent_delegate::delegate = authority,
)]
pub mint_b: Box<InterfaceAccount<'info, Mint>>,

/// CHECK: This account's data is a buffer of TLV data
#[account(
    init,
    space = get_meta_list_size(None),
    seeds = [META_LIST_ACCOUNT_SEED, mint_b.key().as_ref()],
    bump,
    payer = payer,
)]
pub extra_metas_account_mint_b: UncheckedAccount<'info>,

// Mint Authority
/// CHECK: Read only authority
#[account(
    seeds = [
        AUTHORITY_SEED.as_ref(),
    ],
    bump,
)]
pub authority: AccountInfo<'info>,

#[account(mut)]
pub payer: Signer<'info>,
pub token_program: Program<'info, Token2022>,
pub associated_token_program: Program<'info, AssociatedToken>,
pub system_program: Program<'info, System>,

}


impl<'info> CreateEndcoin<'info> {
    pub fn initialize_endcoin(
        &mut self,
    ) -> ProgramResult {
        
        let name: &str;
        let symbol: &str;
        let uri: &str;

                name = "Endcoin";
                symbol = "END";
                uri = "https://arweave.net/K-3ZVae8bb_iMFLTZKuO_7CMNLtOc-bmMafhhn_PeF0";
            
            let cpi_accounts = TokenMetadataInitialize {
                token_program_id: self.token_program.to_account_info(),
                mint: self.mint_a.to_account_info(),
                metadata: self.mint_a.to_account_info(), // metadata account is the mint, since data is stored in mint
                mint_authority: self.authority.to_account_info(),
                update_authority: self.authority.to_account_info(),
            };
            
            let cpi_ctx = CpiContext::new(self.token_program.to_account_info(), cpi_accounts);
            token_metadata_initialize(cpi_ctx, name.to_string(), symbol.to_string(), uri.to_string())?;
            
            
        Ok(())
        }
        
    
    }

    impl<'info> CreateGaiacoin<'info> {
    pub fn initialize_gaiacoin(
        &mut self,
    ) -> ProgramResult {
        
        let name: &str;
        let symbol: &str;
        let uri: &str;

                name = "Gaiacoin";
                symbol = "GAIA";
                uri = "https://arweave.net/K-3ZVae8bb_iMFLTZKuO_7CMNLtOc-bmMafhhn_PeF0";
            
            let cpi_accounts = TokenMetadataInitialize {
                token_program_id: self.token_program.to_account_info(),
                mint: self.mint_b.to_account_info(),
                metadata: self.mint_b.to_account_info(), // metadata account is the mint, since data is stored in mint
                mint_authority: self.authority.to_account_info(),
                update_authority: self.authority.to_account_info(),
            };
            
            let cpi_ctx = CpiContext::new(self.token_program.to_account_info(), cpi_accounts);
            token_metadata_initialize(cpi_ctx, name.to_string(), symbol.to_string(), uri.to_string())?;
            
            
        Ok(())
        }
        
    
    }

    


// pub fn handler(ctx: Context<T>) -> Result<()> {
    
//     // initialize token metadata
//     ctx.accounts.initialize()?;
    

//     token_mint.reload()?;
//     let mint_data = &mut token_mint.to_account_info();(mint_data)?;
//     assert_eq!(metadata.mint, token_mint.key());
//     let metadata_pointer = get_mint_extension_data::<MetadataPointer>(mint_data)?;
//     let mint_key: Option<Pubkey> = Some(token_mint.key());
//     let authority_key: Option<Pubkey> = Some(ctx.accounts.authority.key());
//     assert_eq!(
//         metadata_pointer.metadata_address,
//         OptionalNonZeroPubkey::try_from(mint_key)?
//     );
//     assert_eq!(
//         metadata_pointer.authority,
//         OptionalNonZeroPubkey::try_from(authority_key)?
//     );
//     let permanent_delegate = get_mint_extension_data::<PermanentDelegate>(mint_data)?;
//     assert_eq!(
//         permanent_delegate.delegate,
//         OptionalNonZeroPubkey::try_from(authority_key)?
//     );
//     let close_authority = get_mint_extension_data::<MintCloseAuthority>(mint_data)?;
//     assert_eq!(
//         close_authority.close_authority,
//         OptionalNonZeroPubkey::try_from(authority_key)?
//     );
//     let transfer_hook = get_mint_extension_data::<TransferHook>(mint_data)?;
//     let program_id: Option<Pubkey> = Some(ctx.program_id.key());
//     assert_eq!(
//         transfer_hook.authority,
//         OptionalNonZeroPubkey::try_from(authority_key)?
//     );
//     assert_eq!(
//         transfer_hook.program_id,
//         OptionalNonZeroPubkey::try_from(program_id)?
//     );
//     let group_member_pointer = get_mint_extension_data::<GroupMemberPointer>(mint_data)?;
//     assert_eq!(
//         group_member_pointer.authority,
//         OptionalNonZeroPubkey::try_from(authority_key)?
//     );
//     assert_eq!(
//         group_member_pointer.member_address,
//         OptionalNonZeroPubkey::try_from(mint_key)?
//     );
//     update_account_lamports_to_minimum_balance(
//         token_mint.to_account_info(),
//         ctx.accounts.payer.to_account_info(),
//         ctx.accounts.system_program.to_account_info(),
//     )?;

//     Ok(())
// }