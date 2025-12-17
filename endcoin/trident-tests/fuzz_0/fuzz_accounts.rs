use trident_fuzz::fuzzing::*;

/// Storage for all account addresses used in fuzz testing.
///
/// This struct serves as a centralized repository for account addresses,
/// enabling their reuse across different instruction flows and test scenarios.
///
/// Docs: https://ackee.xyz/trident/docs/latest/trident-api-macro/trident-types/fuzz-accounts/
#[derive(Default)]
pub struct AccountAddresses {
    pub pool: AddressStorage,

    pub claimer: AddressStorage,

    pub to_mint_a_account: AddressStorage,

    pub to_mint_b_account: AddressStorage,

    pub mint_a: AddressStorage,

    pub mint_b: AddressStorage,

    pub reward_vault: AddressStorage,

    pub reward_account_a: AddressStorage,

    pub reward_account_b: AddressStorage,

    pub system_program: AddressStorage,

    pub associated_token_program: AddressStorage,

    pub token_program: AddressStorage,

    pub amm: AddressStorage,

    pub admin: AddressStorage,

    pub authority: AddressStorage,

    pub pool_authority: AddressStorage,

    pub mint_liquidity: AddressStorage,

    pub payer: AddressStorage,

    pub sst: AddressStorage,

    pub pool_account_a: AddressStorage,

    pub pool_account_b: AddressStorage,

    pub depositor_account_liquidity: AddressStorage,

    pub mint_authority: AddressStorage,

    pub feed: AddressStorage,

    pub trader: AddressStorage,

    pub trader_account_a: AddressStorage,

    pub trader_account_b: AddressStorage,

    pub time: AddressStorage,
}
