use anchor_lang::{
    prelude::Result,
    solana_program::{
        account_info::AccountInfo, program::invoke, pubkey::Pubkey, rent::Rent,
        system_instruction::transfer, sysvar::Sysvar,
    },
    Lamports,
};

use anchor_spl::token_interface::spl_token_2022::{
    extension::{BaseStateWithExtensions, Extension, StateWithExtensions},
    state::Mint,
};
use bytemuck::Pod;

use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};

pub fn update_account_lamports_to_minimum_balance<'info>(
    account: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
) -> Result<()> {
    let extra_lamports = Rent::get()?.minimum_balance(account.data_len()) - account.get_lamports();
    if extra_lamports > 0 {
        invoke(
            &transfer(payer.key, account.key, extra_lamports),
            &[payer, account, system_program],
        )?;
    }
    Ok(())
}

pub fn get_mint_extension_data<T: Extension + Pod>(account: &mut AccountInfo) -> Result<T> {
    let mint_data = account.data.borrow();
    let mint_with_extension = StateWithExtensions::<Mint>::unpack(&mint_data)?;
    let extension_data = *mint_with_extension.get_extension::<T>()?;
    Ok(extension_data)
}

pub fn get_meta_list(approve_account: Option<Pubkey>) -> Vec<ExtraAccountMeta> {
    if let Some(approve_account) = approve_account {
        return vec![ExtraAccountMeta {
            discriminator: 0,
            address_config: approve_account.to_bytes(),
            is_signer: false.into(),
            is_writable: true.into(),
        }];
    }
    vec![]
}

pub fn get_meta_list_size(approve_account: Option<Pubkey>) -> usize {
    // safe because it's either 0 or 1
    ExtraAccountMetaList::size_of(get_meta_list(approve_account).len()).unwrap()
}
