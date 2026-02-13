use borsh::BorshSerialize;
use fuzz_accounts::*;
use solana_sdk::account::ReadableAccount;
use solana_sdk::clock::Clock;
use trident_fuzz::fuzzing::*;

mod fuzz_accounts;
mod types;
use types::*;

const ANCHOR_DISCRIMINATOR_BYTES: usize = 8;

// Anchor custom error codes from AmmError.
const AMM_ERROR_UNAUTHORIZED_CLAIMER: u32 = 6009;
const AMM_ERROR_INVALID_POOL_ACCOUNT: u32 = 6019;
const AMM_ERROR_ORACLE_FEED_STALE: u32 = 6021;
const AMM_ERROR_ORACLE_VALUE_MISSING: u32 = 6023;
const AMM_ERROR_PARAMETER_RECENTLY_UPDATED: u32 = 6024;
const AMM_ERROR_PRICE_IMPACT_TOO_HIGH: u32 = 6025;

fn token_2022_program_id() -> Pubkey {
    pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
}

fn associated_token_program_id() -> Pubkey {
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &associated_token_program_id(),
    )
    .0
}

fn token_account_amount(trident: &mut Trident, token_account: &Pubkey) -> u64 {
    let account = trident.get_account(token_account);
    let data = account.data();
    if data.len() < 72 {
        return 0;
    }
    u64::from_le_bytes(
        data[64..72]
            .try_into()
            .expect("token account amount slice should be 8 bytes"),
    )
}

macro_rules! assert_success {
    ($result:expr, $label:expr $(,)?) => {
        assert!(
            $result.is_success(),
            "expected {} success, got: {:?}\nlogs: {}",
            $label,
            $result.get_result(),
            $result.logs()
        );
    };
}

macro_rules! assert_custom_error {
    ($result:expr, $code:expr, $label:expr $(,)?) => {
        assert!(
            $result.is_custom_error_with_code($code),
            "expected {} custom error {}, got: {:?}\nlogs: {}",
            $label,
            $code,
            $result.get_result(),
            $result.logs()
        );
    };
}

#[derive(FuzzTestMethods)]
struct FuzzTest {
    trident: Trident,
    fuzz_accounts: AccountAddresses,

    authority: Pubkey,
    trader: Pubkey,

    amm: Pubkey,
    mint_authority: Pubkey,

    sst: Pubkey,
    oracle_feed: Pubkey,

    pool: Pubkey,
    pool_authority: Pubkey,

    mint_a: Pubkey,
    mint_b: Pubkey,
    mint_liquidity: Pubkey,

    pool_account_a: Pubkey,
    pool_account_b: Pubkey,
    fake_pool_account_a: Pubkey,

    reward_vault: Pubkey,
    reward_account_a: Pubkey,
    reward_account_b: Pubkey,

    trader_account_a: Pubkey,
    trader_account_b: Pubkey,
    depositor_liquidity_account: Pubkey,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        Self {
            trident: Trident::default(),
            fuzz_accounts: AccountAddresses::default(),
            authority: Pubkey::default(),
            trader: Pubkey::default(),
            amm: Pubkey::default(),
            mint_authority: Pubkey::default(),
            sst: Pubkey::default(),
            oracle_feed: Pubkey::default(),
            pool: Pubkey::default(),
            pool_authority: Pubkey::default(),
            mint_a: Pubkey::default(),
            mint_b: Pubkey::default(),
            mint_liquidity: Pubkey::default(),
            pool_account_a: Pubkey::default(),
            pool_account_b: Pubkey::default(),
            fake_pool_account_a: Pubkey::default(),
            reward_vault: Pubkey::default(),
            reward_account_a: Pubkey::default(),
            reward_account_b: Pubkey::default(),
            trader_account_a: Pubkey::default(),
            trader_account_b: Pubkey::default(),
            depositor_liquidity_account: Pubkey::default(),
        }
    }

    #[init]
    fn start(&mut self) {
        self.authority = self.trident.payer().pubkey();
        self.trader = self.authority;
        self.trident.airdrop(&self.authority, 50 * LAMPORTS_PER_SOL);

        (self.amm, _) = self
            .trident
            .find_program_address(&[b"amm".as_ref()], &endcoin::program_id());
        (self.mint_authority, _) = self
            .trident
            .find_program_address(&[b"authority".as_ref()], &endcoin::program_id());

        let create_amm_ix =
            endcoin::CreateAmmInstruction::data(endcoin::CreateAmmInstructionData::new(500))
                .accounts(endcoin::CreateAmmInstructionAccounts::new(
                    self.amm,
                    self.authority,
                    self.authority,
                ))
                .instruction();
        let create_amm_result = self
            .trident
            .process_transaction(&[create_amm_ix], Some("CreateAmm"));
        assert_success!(&create_amm_result, "CreateAmm");

        self.mint_a = self.trident.random_pubkey();
        self.mint_b = self.trident.random_pubkey();
        self.mint_liquidity = self.trident.random_pubkey();

        (self.pool, _) = self.trident.find_program_address(
            &[
                self.amm.as_ref(),
                self.mint_a.as_ref(),
                self.mint_b.as_ref(),
            ],
            &endcoin::program_id(),
        );
        (self.pool_authority, _) = self.trident.find_program_address(
            &[
                self.amm.as_ref(),
                self.mint_a.as_ref(),
                self.mint_b.as_ref(),
                b"pool-authority".as_ref(),
            ],
            &endcoin::program_id(),
        );

        let mut init_mints_ixs = Vec::new();
        init_mints_ixs.extend(self.trident.initialize_mint_2022(
            &self.authority,
            &self.mint_a,
            6,
            &self.authority,
            None,
            &[],
        ));
        init_mints_ixs.extend(self.trident.initialize_mint_2022(
            &self.authority,
            &self.mint_b,
            6,
            &self.authority,
            None,
            &[],
        ));
        init_mints_ixs.extend(self.trident.initialize_mint_2022(
            &self.authority,
            &self.mint_liquidity,
            6,
            &self.pool_authority,
            None,
            &[],
        ));
        let init_mints_result = self
            .trident
            .process_transaction(&init_mints_ixs, Some("InitializeMints"));
        assert_success!(&init_mints_result, "InitializeMints");

        let create_pool_ix =
            endcoin::CreatePoolInstruction::data(endcoin::CreatePoolInstructionData::new())
                .accounts(endcoin::CreatePoolInstructionAccounts::new(
                    self.amm,
                    self.pool,
                    self.pool_authority,
                    self.mint_liquidity,
                    self.mint_a,
                    self.mint_b,
                    self.authority,
                ))
                .instruction();
        let create_pool_result = self
            .trident
            .process_transaction(&[create_pool_ix], Some("CreatePool"));
        assert_success!(&create_pool_result, "CreatePool");

        self.pool_account_a =
            ata_address(&self.pool_authority, &self.mint_a, &token_2022_program_id());
        self.pool_account_b =
            ata_address(&self.pool_authority, &self.mint_b, &token_2022_program_id());
        self.depositor_liquidity_account = ata_address(
            &self.pool_authority,
            &self.mint_liquidity,
            &token_2022_program_id(),
        );

        let create_pool_token_accounts_ix = endcoin::CreateTokenAccountsInstruction::data(
            endcoin::CreateTokenAccountsInstructionData::new(),
        )
        .accounts(endcoin::CreateTokenAccountsInstructionAccounts::new(
            self.pool,
            self.pool_account_a,
            self.pool_account_b,
            self.pool_authority,
            self.amm,
            self.mint_a,
            self.mint_b,
            self.authority,
        ))
        .instruction();
        let create_pool_token_accounts_result = self.trident.process_transaction(
            &[create_pool_token_accounts_ix],
            Some("CreatePoolTokenAccounts"),
        );
        assert_success!(
            &create_pool_token_accounts_result,
            "CreatePoolTokenAccounts",
        );

        (self.reward_vault, _) = self.trident.find_program_address(
            &[
                self.pool.as_ref(),
                self.mint_a.as_ref(),
                self.mint_b.as_ref(),
                b"reward-vault".as_ref(),
            ],
            &endcoin::program_id(),
        );
        self.reward_account_a =
            ata_address(&self.reward_vault, &self.mint_a, &token_2022_program_id());
        self.reward_account_b =
            ata_address(&self.reward_vault, &self.mint_b, &token_2022_program_id());

        let create_reward_vault_ix = endcoin::CreateRewardVaultInstruction::data(
            endcoin::CreateRewardVaultInstructionData::new(self.authority),
        )
        .accounts(endcoin::CreateRewardVaultInstructionAccounts::new(
            self.reward_vault,
            self.pool,
            self.amm,
            self.mint_a,
            self.mint_b,
            self.authority,
        ))
        .instruction();
        let create_reward_vault_result = self
            .trident
            .process_transaction(&[create_reward_vault_ix], Some("CreateRewardVault"));
        assert_success!(&create_reward_vault_result, "CreateRewardVault");

        let create_reward_token_accounts_ix = endcoin::CreateRewardTokenAccountsInstruction::data(
            endcoin::CreateRewardTokenAccountsInstructionData::new(),
        )
        .accounts(endcoin::CreateRewardTokenAccountsInstructionAccounts::new(
            self.pool,
            self.amm,
            self.reward_account_a,
            self.reward_account_b,
            self.reward_vault,
            self.mint_a,
            self.mint_b,
            self.authority,
        ))
        .instruction();
        let create_reward_token_accounts_result = self.trident.process_transaction(
            &[create_reward_token_accounts_ix],
            Some("CreateRewardTokenAccounts"),
        );
        assert_success!(
            &create_reward_token_accounts_result,
            "CreateRewardTokenAccounts",
        );

        // Trader ATAs used in swap and claim paths.
        self.trader_account_a = ata_address(&self.trader, &self.mint_a, &token_2022_program_id());
        self.trader_account_b = ata_address(&self.trader, &self.mint_b, &token_2022_program_id());
        let mut trader_atas_ixs = Vec::new();
        trader_atas_ixs.extend(self.trident.initialize_associated_token_account_2022(
            &self.authority,
            &self.mint_a,
            &self.trader,
            &[],
        ));
        trader_atas_ixs.extend(self.trident.initialize_associated_token_account_2022(
            &self.authority,
            &self.mint_b,
            &self.trader,
            &[],
        ));
        let trader_atas_result = self
            .trident
            .process_transaction(&trader_atas_ixs, Some("CreateTraderATAs"));
        assert_success!(&trader_atas_result, "CreateTraderATAs");

        // Create an extra non-canonical reserve account with the same mint/owner combo.
        self.fake_pool_account_a = self.trident.random_pubkey();
        let fake_account_ixs = self.trident.initialize_token_account_2022(
            &self.authority,
            &self.fake_pool_account_a,
            &self.mint_a,
            &self.pool_authority,
            &[],
        );
        let fake_account_result = self
            .trident
            .process_transaction(&fake_account_ixs, Some("CreateFakePoolAccount"));
        assert_success!(&fake_account_result, "CreateFakePoolAccount");

        self.oracle_feed = self.trident.random_pubkey();
        (self.sst, _) = self.trident.find_program_address(
            &[b"sea-surface-temperature".as_ref(), self.amm.as_ref()],
            &endcoin::program_id(),
        );

        let create_sst_ix = endcoin::CreateSstInstruction::data(
            endcoin::CreateSstInstructionData::new(self.oracle_feed),
        )
        .accounts(endcoin::CreateSstInstructionAccounts::new(
            self.amm,
            self.sst,
            self.authority,
            self.authority,
        ))
        .instruction();
        let create_sst_result = self
            .trident
            .process_transaction(&[create_sst_ix], Some("CreateSst"));
        assert_success!(&create_sst_result, "CreateSst");

        let seed_balances_ixs = vec![
            self.trident.mint_to_2022(
                &self.pool_account_a,
                &self.mint_a,
                &self.authority,
                1_000_000,
            ),
            self.trident.mint_to_2022(
                &self.pool_account_b,
                &self.mint_b,
                &self.authority,
                1_000_000,
            ),
            self.trident.mint_to_2022(
                &self.reward_account_a,
                &self.mint_a,
                &self.authority,
                500_000,
            ),
            self.trident.mint_to_2022(
                &self.reward_account_b,
                &self.mint_b,
                &self.authority,
                500_000,
            ),
            self.trident.mint_to_2022(
                &self.trader_account_a,
                &self.mint_a,
                &self.authority,
                250_000,
            ),
            self.trident.mint_to_2022(
                &self.trader_account_b,
                &self.mint_b,
                &self.authority,
                250_000,
            ),
        ];
        let seed_balances_result = self
            .trident
            .process_transaction(&seed_balances_ixs, Some("SeedTokenBalances"));
        assert_success!(&seed_balances_result, "SeedTokenBalances");

        self.ensure_swap_enabled();
    }

    fn write_sst(&mut self, temperature: f64, last_updated_slot: u64, last_updated_unix_ts: i64) {
        let mut sst: SST = self
            .trident
            .get_account_with_type(&self.sst, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("sst account should deserialize");
        sst.temperature = temperature;
        sst.created = true;
        sst.oracle_feed = self.oracle_feed;
        sst.last_updated_slot = last_updated_slot;
        sst.last_updated_unix_timestamp = last_updated_unix_ts;

        let mut account = self.trident.get_account(&self.sst);
        let mut encoded = Vec::new();
        sst.serialize(&mut encoded)
            .expect("sst account serialization should succeed");

        let mut data = account.data().to_vec();
        let start = ANCHOR_DISCRIMINATOR_BYTES;
        let end = start + encoded.len();
        assert!(
            data.len() >= end,
            "sst account too small: len={} expected_at_least={}",
            data.len(),
            end
        );
        data[start..end].copy_from_slice(&encoded);
        account.set_data_from_slice(&data);
        self.trident.set_account_custom(&self.sst, &account);
    }

    fn ensure_swap_enabled(&mut self) {
        let amm: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("amm account should deserialize");

        let mut clock = self.trident.get_sysvar::<Clock>();
        let min_slot = amm.last_fee_update_slot.saturating_add(2);
        if clock.slot < min_slot {
            self.trident.warp_to_slot(min_slot);
            clock = self.trident.get_sysvar::<Clock>();
        }

        let fresh_ts = clock.unix_timestamp.saturating_sub(1).max(1);
        self.write_sst(21.0, clock.slot.saturating_sub(2), fresh_ts);
    }

    fn ensure_trader_balance_a(&mut self, min_amount: u64) {
        let current = token_account_amount(&mut self.trident, &self.trader_account_a);
        if current >= min_amount {
            return;
        }

        let top_up_amount = min_amount - current + 10_000;
        let top_up_ix = self.trident.mint_to_2022(
            &self.trader_account_a,
            &self.mint_a,
            &self.authority,
            top_up_amount,
        );
        let top_up_result = self
            .trident
            .process_transaction(&[top_up_ix], Some("TopUpTraderA"));
        assert_success!(&top_up_result, "TopUpTraderA");
    }

    fn ensure_reward_balances(&mut self, min_amount: u64) {
        let bal_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let bal_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        if bal_a >= min_amount && bal_b >= min_amount {
            return;
        }

        let mut ixs = Vec::new();
        if bal_a < min_amount {
            ixs.push(self.trident.mint_to_2022(
                &self.reward_account_a,
                &self.mint_a,
                &self.authority,
                min_amount - bal_a + 10_000,
            ));
        }
        if bal_b < min_amount {
            ixs.push(self.trident.mint_to_2022(
                &self.reward_account_b,
                &self.mint_b,
                &self.authority,
                min_amount - bal_b + 10_000,
            ));
        }

        let top_up_result = self
            .trident
            .process_transaction(&ixs, Some("TopUpRewardVault"));
        assert_success!(&top_up_result, "TopUpRewardVault");
    }

    fn set_reward_whitelist(&mut self, whitelist_authority: Pubkey) {
        let ix = endcoin::UpdateRewardWhitelistInstruction::data(
            endcoin::UpdateRewardWhitelistInstructionData::new(whitelist_authority),
        )
        .accounts(endcoin::UpdateRewardWhitelistInstructionAccounts::new(
            self.reward_vault,
            self.pool,
            self.amm,
            self.mint_a,
            self.mint_b,
            self.authority,
        ))
        .instruction();

        let result = self
            .trident
            .process_transaction(&[ix], Some("UpdateRewardWhitelist"));
        assert_success!(&result, "UpdateRewardWhitelist");
    }

    fn swap_ix(
        &self,
        pool_account_a: Pubkey,
        pool_account_b: Pubkey,
        input_amount: u64,
    ) -> Instruction {
        endcoin::SwapInstruction::data(endcoin::SwapInstructionData::new(true, input_amount, 0))
            .accounts(endcoin::SwapInstructionAccounts::new(
                self.amm,
                self.pool_authority,
                self.trader,
                self.mint_a,
                self.mint_b,
                self.pool,
                pool_account_a,
                pool_account_b,
                self.sst,
                self.trader_account_a,
                self.trader_account_b,
                self.authority,
            ))
            .instruction()
    }

    #[flow]
    fn flow_deposit_liquidity_requires_oracle_pull(&mut self) {
        // Force a missing oracle state.
        self.write_sst(21.0, 0, 0);

        let ix = endcoin::DepositLiquidityInstruction::data(
            endcoin::DepositLiquidityInstructionData::new(),
        )
        .accounts(endcoin::DepositLiquidityInstructionAccounts::new(
            self.amm,
            self.pool,
            self.pool_authority,
            self.sst,
            self.authority,
            self.mint_liquidity,
            self.mint_a,
            self.mint_b,
            self.pool_account_a,
            self.pool_account_b,
            self.depositor_liquidity_account,
            self.mint_authority,
        ))
        .instruction();

        let result = self
            .trident
            .process_transaction(&[ix], Some("DepositLiquidityWithoutOracle"));
        assert_custom_error!(
            &result,
            AMM_ERROR_ORACLE_VALUE_MISSING,
            "DepositLiquidityWithoutOracle",
        );

        self.ensure_swap_enabled();
    }

    #[flow]
    fn flow_claim_reward_requires_whitelist_signer(&mut self) {
        self.ensure_reward_balances(5_000);

        let random_whitelist = self.trident.random_pubkey();
        self.set_reward_whitelist(random_whitelist);

        let claim_amount_a = 1_000;
        let claim_amount_b = 1_000;

        let fail_ix = endcoin::ClaimRewardInstruction::data(
            endcoin::ClaimRewardInstructionData::new(claim_amount_a, claim_amount_b),
        )
        .accounts(endcoin::ClaimRewardInstructionAccounts::new(
            self.pool,
            self.trader,
            self.trader_account_a,
            self.trader_account_b,
            self.mint_a,
            self.mint_b,
            self.reward_vault,
            self.authority,
            self.reward_account_a,
            self.reward_account_b,
        ))
        .instruction();

        let fail_result = self
            .trident
            .process_transaction(&[fail_ix], Some("ClaimRewardUnauthorized"));
        assert_custom_error!(
            &fail_result,
            AMM_ERROR_UNAUTHORIZED_CLAIMER,
            "ClaimRewardUnauthorized",
        );

        self.set_reward_whitelist(self.authority);

        let before_reward_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let before_reward_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        let before_trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let before_trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);

        let ok_ix = endcoin::ClaimRewardInstruction::data(
            endcoin::ClaimRewardInstructionData::new(claim_amount_a, claim_amount_b),
        )
        .accounts(endcoin::ClaimRewardInstructionAccounts::new(
            self.pool,
            self.trader,
            self.trader_account_a,
            self.trader_account_b,
            self.mint_a,
            self.mint_b,
            self.reward_vault,
            self.authority,
            self.reward_account_a,
            self.reward_account_b,
        ))
        .instruction();

        let ok_result = self
            .trident
            .process_transaction(&[ok_ix], Some("ClaimRewardAuthorized"));
        assert_success!(&ok_result, "ClaimRewardAuthorized");

        let after_reward_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let after_reward_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        let after_trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let after_trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);

        assert_eq!(after_reward_a, before_reward_a - claim_amount_a);
        assert_eq!(after_reward_b, before_reward_b - claim_amount_b);
        assert_eq!(after_trader_a, before_trader_a + claim_amount_a);
        assert_eq!(after_trader_b, before_trader_b + claim_amount_b);
    }

    #[flow]
    fn flow_swap_rejects_noncanonical_pool_accounts(&mut self) {
        self.ensure_swap_enabled();
        self.ensure_trader_balance_a(5_000);

        let ix = self.swap_ix(self.fake_pool_account_a, self.pool_account_b, 1_000);
        let result = self
            .trident
            .process_transaction(&[ix], Some("SwapWithSpoofedReserve"));

        assert_custom_error!(
            &result,
            AMM_ERROR_INVALID_POOL_ACCOUNT,
            "SwapWithSpoofedReserve",
        );
    }

    #[flow]
    fn flow_swap_rejects_recent_fee_updates(&mut self) {
        self.ensure_swap_enabled();
        self.ensure_trader_balance_a(20_000);

        let new_fee = self.trident.random_from_range(5u16..=1_000u16);
        let update_fee_ix =
            endcoin::UpdateFeeInstruction::data(endcoin::UpdateFeeInstructionData::new(new_fee))
                .accounts(endcoin::UpdateFeeInstructionAccounts::new(
                    self.amm,
                    self.authority,
                ))
                .instruction();

        let update_fee_result = self
            .trident
            .process_transaction(&[update_fee_ix], Some("UpdateFee"));
        assert_success!(&update_fee_result, "UpdateFee");

        let amm: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("amm account should deserialize after update fee");

        // Keep fee-update cooldown active by forcing the clock to the exact update slot.
        self.trident.warp_to_slot(amm.last_fee_update_slot);
        let clock = self.trident.get_sysvar::<Clock>();
        self.write_sst(
            21.0,
            clock.slot.saturating_sub(2),
            clock.unix_timestamp.saturating_sub(1).max(1),
        );

        let blocked_swap_ix = self.swap_ix(self.pool_account_a, self.pool_account_b, 10_000);
        let blocked_result = self
            .trident
            .process_transaction(&[blocked_swap_ix], Some("SwapDuringFeeCooldown"));
        assert_custom_error!(
            &blocked_result,
            AMM_ERROR_PARAMETER_RECENTLY_UPDATED,
            "SwapDuringFeeCooldown",
        );

        // Move one slot ahead so cooldown expires.
        self.trident
            .warp_to_slot(amm.last_fee_update_slot.saturating_add(1));
        self.ensure_swap_enabled();

        let allowed_swap_ix = self.swap_ix(self.pool_account_a, self.pool_account_b, 5_000);
        let allowed_result = self
            .trident
            .process_transaction(&[allowed_swap_ix], Some("SwapAfterFeeCooldown"));
        assert_success!(&allowed_result, "SwapAfterFeeCooldown");
    }

    #[flow]
    fn flow_swap_rejects_stale_oracle_values(&mut self) {
        self.ensure_swap_enabled();
        self.ensure_trader_balance_a(5_000);

        let clock = self.trident.get_sysvar::<Clock>();
        self.write_sst(
            21.0,
            clock.slot.saturating_sub(2),
            clock.unix_timestamp.saturating_sub(901),
        );

        let ix = self.swap_ix(self.pool_account_a, self.pool_account_b, 2_000);
        let result = self
            .trident
            .process_transaction(&[ix], Some("SwapWithStaleOracle"));

        assert_custom_error!(&result, AMM_ERROR_ORACLE_FEED_STALE, "SwapWithStaleOracle");

        self.ensure_swap_enabled();
    }

    #[flow]
    fn flow_swap_rejects_price_impact_spikes(&mut self) {
        self.ensure_swap_enabled();

        let reserve_in = token_account_amount(&mut self.trident, &self.pool_account_a);
        let input_amount = reserve_in.max(1);
        self.ensure_trader_balance_a(input_amount.saturating_add(5_000));

        let ix = self.swap_ix(self.pool_account_a, self.pool_account_b, input_amount);
        let result = self
            .trident
            .process_transaction(&[ix], Some("SwapPriceImpactGuard"));

        assert_custom_error!(
            &result,
            AMM_ERROR_PRICE_IMPACT_TOO_HIGH,
            "SwapPriceImpactGuard",
        );
    }

    #[flow]
    fn flow_swap_happy_path_still_works(&mut self) {
        self.ensure_swap_enabled();
        self.ensure_trader_balance_a(20_000);

        let before_trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let before_trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);
        let before_pool_a = token_account_amount(&mut self.trident, &self.pool_account_a);
        let before_pool_b = token_account_amount(&mut self.trident, &self.pool_account_b);

        let input = 10_000;
        let ix = self.swap_ix(self.pool_account_a, self.pool_account_b, input);
        let result = self
            .trident
            .process_transaction(&[ix], Some("SwapHappyPath"));
        assert_success!(&result, "SwapHappyPath");

        let after_trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let after_trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);
        let after_pool_a = token_account_amount(&mut self.trident, &self.pool_account_a);
        let after_pool_b = token_account_amount(&mut self.trident, &self.pool_account_b);

        assert_eq!(after_trader_a, before_trader_a - input);
        assert!(after_trader_b > before_trader_b);
        assert_eq!(after_pool_a, before_pool_a + input);
        assert!(after_pool_b < before_pool_b);
    }

    #[end]
    fn end(&mut self) {}
}

fn main() {
    // 250 iterations x 25 instruction invocations gives broad state exploration
    // while keeping local feedback loop fast enough for CI.
    FuzzTest::fuzz(250, 25);
}
