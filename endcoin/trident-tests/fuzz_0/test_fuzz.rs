use fuzz_accounts::*;
use trident_fuzz::fuzzing::*;
mod fuzz_accounts;
mod types;
use types::*;

use solana_sdk::account::AccountSharedData;
use solana_sdk::rent::Rent;
use solana_sdk::system_instruction;

const ANCHOR_DISCRIMINATOR_BYTES: usize = 8;

// Anchor custom errors start at 6000 by default.
const AMM_ERROR_INVALID_FEE: u32 = 6000;
const AMM_ERROR_INVALID_MINT: u32 = 6001;
const AMM_ERROR_DEPOSIT_TOO_SMALL: u32 = 6002;
const AMM_ERROR_OUTPUT_TOO_SMALL: u32 = 6003;
const AMM_ERROR_UNAUTHORIZED_ADMIN: u32 = 6006;
const AMM_ERROR_NOT_CREATED: u32 = 6007;
const AMM_ERROR_NOT_SIGNER: u32 = 6008;
const AMM_ERROR_UNAUTHORIZED_CLAIMER: u32 = 6009;
const AMM_ERROR_INSUFFICIENT_REWARD: u32 = 6010;
const AMM_ERROR_INPUT_AMOUNT_TOO_SMALL: u32 = 6015;
const AMM_ERROR_INSUFFICIENT_BALANCE: u32 = 6016;
const AMM_ERROR_INSUFFICIENT_LIQUIDITY: u32 = 6017;
const AMM_ERROR_INVALID_TEMPERATURE: u32 = 6018;

const MINT_ACCOUNT_LEN: u64 = 82;

fn token_2022_program_id() -> Pubkey {
    pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
}

fn associated_token_program_id() -> Pubkey {
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
}

fn system_program_id() -> Pubkey {
    pubkey!("11111111111111111111111111111111")
}

fn pull_feed_pubkey() -> Pubkey {
    pubkey!("GhCs7zhha7kTyt8EiaWaBT5DREt23GnoPnqa7AU4yv1y")
}

fn ata_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &associated_token_program_id(),
    )
    .0
}

fn create_ata_idempotent_ix(
    payer: Pubkey,
    ata: Pubkey,
    owner: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
) -> Instruction {
    // borsh enum discriminant: CreateIdempotent = 1u8
    Instruction {
        program_id: associated_token_program_id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(system_program_id(), false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: vec![1u8],
    }
}

fn token2022_initialize_mint2_ix(mint: Pubkey, mint_authority: Pubkey, decimals: u8) -> Instruction {
    // spl-token-2022 TokenInstruction::InitializeMint2 tag = 20
    let mut data = Vec::with_capacity(1 + 1 + 32 + 1);
    data.push(20u8);
    data.push(decimals);
    data.extend_from_slice(mint_authority.as_ref());
    // freeze_authority: COption<Pubkey> in instruction encoding (0/1 + pubkey)
    data.push(0u8);

    Instruction {
        program_id: token_2022_program_id(),
        accounts: vec![AccountMeta::new(mint, false)],
        data,
    }
}

fn token2022_mint_to_checked_ix(
    mint: Pubkey,
    to: Pubkey,
    authority: Pubkey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    // spl-token-2022 TokenInstruction::MintToChecked tag = 14
    let mut data = Vec::with_capacity(1 + 8 + 1);
    data.push(14u8);
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);

    Instruction {
        program_id: token_2022_program_id(),
        accounts: vec![
            AccountMeta::new(mint, false),
            AccountMeta::new(to, false),
            AccountMeta::new_readonly(authority, true),
        ],
        data,
    }
}

fn token_account_fields(trident: &mut Trident, token_account: &Pubkey) -> Option<(Pubkey, Pubkey, u64)> {
    let account = trident.get_account(token_account);
    let data = account.data();
    if data.len() < 72 {
        return None;
    }

    let mint = Pubkey::try_from(&data[0..32]).ok()?;
    let owner = Pubkey::try_from(&data[32..64]).ok()?;
    let amount = u64::from_le_bytes(data[64..72].try_into().ok()?);
    Some((mint, owner, amount))
}

fn token_account_amount(trident: &mut Trident, token_account: &Pubkey) -> u64 {
    token_account_fields(trident, token_account)
        .map(|(_, _, amount)| amount)
        .unwrap_or(0)
}

fn assert_token_account_mint_owner(
    trident: &mut Trident,
    token_account: &Pubkey,
    expected_mint: Pubkey,
    expected_owner: Pubkey,
) {
    let (mint, owner, _) = token_account_fields(trident, token_account).unwrap_or_else(|| {
        panic!(
            "token account missing or too small: {} (expected mint={}, owner={})",
            token_account, expected_mint, expected_owner
        )
    });
    assert_eq!(mint, expected_mint, "token account mint mismatch: {token_account}");
    assert_eq!(owner, expected_owner, "token account owner mismatch: {token_account}");
}

#[derive(FuzzTestMethods)]
struct FuzzTest {
    /// Trident client for interacting with the Solana program
    trident: Trident,
    /// Storage for all account addresses used in fuzz testing
    fuzz_accounts: AccountAddresses,

    amm: Pubkey,
    current_admin: Pubkey,
    authority: Pubkey,
    current_fee: u16,

    sst: Pubkey,
    time: Pubkey,
    feed: Pubkey,

    // Emissions + rewards setup (uses program PDAs to mint)
    mint_authority: Pubkey,
    pool_emissions: Pubkey,
    pool_authority_emissions: Pubkey,
    mint_a_emissions: Pubkey,
    mint_b_emissions: Pubkey,
    mint_liquidity_emissions: Pubkey,
    pool_account_a_emissions: Pubkey,
    pool_account_b_emissions: Pubkey,
    depositor_liquidity_emissions: Pubkey,
    reward_vault: Pubkey,
    reward_account_a: Pubkey,
    reward_account_b: Pubkey,
    claimer: Pubkey,
    claimer_account_a: Pubkey,
    claimer_account_b: Pubkey,

    // Swap setup (mints controlled by test to seed balances)
    pool_swap: Pubkey,
    pool_authority_swap: Pubkey,
    mint_a_swap: Pubkey,
    mint_b_swap: Pubkey,
    mint_liquidity_swap: Pubkey,
    pool_account_a_swap: Pubkey,
    pool_account_b_swap: Pubkey,
    trader: Pubkey,
    trader_account_a: Pubkey,
    trader_account_b: Pubkey,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        Self {
            trident: Trident::default(),
            fuzz_accounts: AccountAddresses::default(),
            amm: Pubkey::default(),
            current_admin: Pubkey::default(),
            authority: Pubkey::default(),
            current_fee: 0,
            sst: Pubkey::default(),
            time: Pubkey::default(),
            feed: Pubkey::default(),
            mint_authority: Pubkey::default(),
            pool_emissions: Pubkey::default(),
            pool_authority_emissions: Pubkey::default(),
            mint_a_emissions: Pubkey::default(),
            mint_b_emissions: Pubkey::default(),
            mint_liquidity_emissions: Pubkey::default(),
            pool_account_a_emissions: Pubkey::default(),
            pool_account_b_emissions: Pubkey::default(),
            depositor_liquidity_emissions: Pubkey::default(),
            reward_vault: Pubkey::default(),
            reward_account_a: Pubkey::default(),
            reward_account_b: Pubkey::default(),
            claimer: Pubkey::default(),
            claimer_account_a: Pubkey::default(),
            claimer_account_b: Pubkey::default(),
            pool_swap: Pubkey::default(),
            pool_authority_swap: Pubkey::default(),
            mint_a_swap: Pubkey::default(),
            mint_b_swap: Pubkey::default(),
            mint_liquidity_swap: Pubkey::default(),
            pool_account_a_swap: Pubkey::default(),
            pool_account_b_swap: Pubkey::default(),
            trader: Pubkey::default(),
            trader_account_a: Pubkey::default(),
            trader_account_b: Pubkey::default(),
        }
    }

    #[init]
    fn start(&mut self) {
        // Use Trident's default payer as the system-funded authority.
        self.authority = self.trident.payer().pubkey();
        self.trident.airdrop(&self.authority, 10 * LAMPORTS_PER_SOL);

        // Keep admin == authority so admin-gated instructions can be exercised.
        self.current_admin = self.authority;

        // Derive the canonical AMM PDA.
        (self.amm, _) = self
            .trident
            .find_program_address(&[b"amm".as_ref()], &endcoin::program_id());

        // Test: CreateAmm rejects invalid fee.
        let invalid_fee: u16 = self.trident.random_from_range(0u16..=4u16);
        let invalid_ix = endcoin::CreateAmmInstruction::data(
            endcoin::CreateAmmInstructionData::new(invalid_fee),
        )
        .accounts(endcoin::CreateAmmInstructionAccounts::new(
            self.amm,
            self.current_admin,
            self.authority,
        ))
        .instruction();

        let invalid_result = self
            .trident
            .process_transaction(&[invalid_ix], Some("CreateAmmInvalidFee"));
        assert!(
            invalid_result.is_custom_error_with_code(AMM_ERROR_INVALID_FEE),
            "expected InvalidFee({AMM_ERROR_INVALID_FEE}), got: {:?}\nlogs: {}",
            invalid_result.get_result(),
            invalid_result.logs()
        );

        // Test: CreateAmm succeeds with valid fee and initializes state.
        self.current_fee = self.trident.random_from_range(5u16..=9999u16);
        let create_ix = endcoin::CreateAmmInstruction::data(endcoin::CreateAmmInstructionData::new(
            self.current_fee,
        ))
        .accounts(endcoin::CreateAmmInstructionAccounts::new(
            self.amm,
            self.current_admin,
            self.authority,
        ))
        .instruction();

        let create_result = self
            .trident
            .process_transaction(&[create_ix], Some("CreateAmmValidFee"));
        assert!(
            create_result.is_success(),
            "expected CreateAmm success, got: {:?}\nlogs: {}",
            create_result.get_result(),
            create_result.logs()
        );

        let amm_state: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("AMM account should deserialize after CreateAmm");
        assert_eq!(amm_state.admin, self.current_admin);
        assert_eq!(amm_state.fee, self.current_fee);
        assert!(amm_state.created);

        // Seed the fixed PullFeed account so the account exists in SVM.
        self.feed = pull_feed_pubkey();
        let mut feed_account = AccountSharedData::default();
        feed_account.set_lamports(1);
        self.trident.set_account_custom(&self.feed, &feed_account);

        // Create SST (needed by Swap).
        (self.sst, _) = self.trident.find_program_address(
            &[b"sea-surface-temperature".as_ref(), self.authority.as_ref()],
            &endcoin::program_id(),
        );
        let sst_ix = endcoin::CreateSstInstruction::data(endcoin::CreateSstInstructionData::new())
            .accounts(endcoin::CreateSstInstructionAccounts::new(
                self.sst,
                self.authority,
            ))
            .instruction();
        let sst_result = self.trident.process_transaction(&[sst_ix], Some("CreateSst"));
        assert!(
            sst_result.is_success(),
            "expected CreateSst success, got: {:?}\nlogs: {}",
            sst_result.get_result(),
            sst_result.logs()
        );
        let sst_state: SST = self
            .trident
            .get_account_with_type(&self.sst, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("SST account should deserialize after CreateSst");
        assert!(sst_state.created);
        assert!((sst_state.temperature - 21.0).abs() < f64::EPSILON);

        // Derive mint authority PDA used by DepositLiquidity / DepositRewards.
        (self.mint_authority, _) =
            self.trident
                .find_program_address(&[b"authority".as_ref()], &endcoin::program_id());

        // ---------------------------------------------------------------------
        // Emissions pool setup (CreatePool, CreateTokenAccounts, CreateRewardVault, CreateRewardTokenAccounts)
        // ---------------------------------------------------------------------
        let payer = self.authority;
        let rent = Rent::default();
        let decimals: u8 = 6;

        // Create token-2022 mints used by emissions flows.
        self.mint_a_emissions = self.trident.random_pubkey();
        self.mint_b_emissions = self.trident.random_pubkey();
        let create_mints_ixs = vec![
            system_instruction::create_account(
                &payer,
                &self.mint_a_emissions,
                rent.minimum_balance(MINT_ACCOUNT_LEN as usize),
                MINT_ACCOUNT_LEN,
                &token_2022_program_id(),
            ),
            token2022_initialize_mint2_ix(self.mint_a_emissions, self.mint_authority, decimals),
            system_instruction::create_account(
                &payer,
                &self.mint_b_emissions,
                rent.minimum_balance(MINT_ACCOUNT_LEN as usize),
                MINT_ACCOUNT_LEN,
                &token_2022_program_id(),
            ),
            token2022_initialize_mint2_ix(self.mint_b_emissions, self.mint_authority, decimals),
        ];
        let mints_result = self.trident.process_transaction(&create_mints_ixs, None);
        assert!(
            mints_result.is_success(),
            "expected emissions mints setup success, got: {:?}\nlogs: {}",
            mints_result.get_result(),
            mints_result.logs()
        );

        // Derive pool PDA/authority for this mint pair.
        (self.pool_emissions, _) = self.trident.find_program_address(
            &[
                self.amm.as_ref(),
                self.mint_a_emissions.as_ref(),
                self.mint_b_emissions.as_ref(),
            ],
            &endcoin::program_id(),
        );
        (self.pool_authority_emissions, _) = self.trident.find_program_address(
            &[
                self.amm.as_ref(),
                self.mint_a_emissions.as_ref(),
                self.mint_b_emissions.as_ref(),
                b"pool-authority".as_ref(),
            ],
            &endcoin::program_id(),
        );

        // Liquidity mint must be controlled by pool_authority (program signs with it).
        self.mint_liquidity_emissions = self.trident.random_pubkey();
        let liquidity_mint_ixs = vec![
            system_instruction::create_account(
                &payer,
                &self.mint_liquidity_emissions,
                rent.minimum_balance(MINT_ACCOUNT_LEN as usize),
                MINT_ACCOUNT_LEN,
                &token_2022_program_id(),
            ),
            token2022_initialize_mint2_ix(
                self.mint_liquidity_emissions,
                self.pool_authority_emissions,
                decimals,
            ),
        ];
        let liquidity_mint_result = self.trident.process_transaction(&liquidity_mint_ixs, None);
        assert!(
            liquidity_mint_result.is_success(),
            "expected liquidity mint setup success, got: {:?}\nlogs: {}",
            liquidity_mint_result.get_result(),
            liquidity_mint_result.logs()
        );

        // CreatePool (also tests InvalidMint path once).
        let invalid_pool_ix = endcoin::CreatePoolInstruction::data(
            endcoin::CreatePoolInstructionData::new(),
        )
        .accounts(endcoin::CreatePoolInstructionAccounts::new(
            self.amm,
            // Pool PDA for the invalid pair (mint_a == mint_b).
            self.trident
                .find_program_address(
                    &[
                        self.amm.as_ref(),
                        self.mint_a_emissions.as_ref(),
                        self.mint_a_emissions.as_ref(),
                    ],
                    &endcoin::program_id(),
                )
                .0,
            self.trident
                .find_program_address(
                    &[
                        self.amm.as_ref(),
                        self.mint_a_emissions.as_ref(),
                        self.mint_a_emissions.as_ref(),
                        b"pool-authority".as_ref(),
                    ],
                    &endcoin::program_id(),
                )
                .0,
            self.mint_liquidity_emissions,
            self.mint_a_emissions,
            self.mint_a_emissions,
            self.authority,
        ))
        .instruction();
        let invalid_pool_result = self
            .trident
            .process_transaction(&[invalid_pool_ix], Some("CreatePoolInvalidMint"));
        assert!(
            invalid_pool_result.is_custom_error_with_code(AMM_ERROR_INVALID_MINT),
            "expected InvalidMint({AMM_ERROR_INVALID_MINT}), got: {:?}\nlogs: {}",
            invalid_pool_result.get_result(),
            invalid_pool_result.logs()
        );

        let pool_ix =
            endcoin::CreatePoolInstruction::data(endcoin::CreatePoolInstructionData::new())
                .accounts(endcoin::CreatePoolInstructionAccounts::new(
                    self.amm,
                    self.pool_emissions,
                    self.pool_authority_emissions,
                    self.mint_liquidity_emissions,
                    self.mint_a_emissions,
                    self.mint_b_emissions,
                    self.authority,
                ))
                .instruction();
        let pool_result = self.trident.process_transaction(&[pool_ix], Some("CreatePool"));
        assert!(
            pool_result.is_success(),
            "expected CreatePool success, got: {:?}\nlogs: {}",
            pool_result.get_result(),
            pool_result.logs()
        );
        let pool_state: Pool = self
            .trident
            .get_account_with_type(&self.pool_emissions, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("Pool should deserialize after CreatePool");
        assert_eq!(pool_state.amm, self.amm);
        assert_eq!(pool_state.mint_a, self.mint_a_emissions);
        assert_eq!(pool_state.mint_b, self.mint_b_emissions);

        // CreateTokenAccounts for the pool authority.
        self.pool_account_a_emissions =
            ata_address(&self.pool_authority_emissions, &self.mint_a_emissions, &token_2022_program_id());
        self.pool_account_b_emissions =
            ata_address(&self.pool_authority_emissions, &self.mint_b_emissions, &token_2022_program_id());
        self.depositor_liquidity_emissions =
            ata_address(&self.pool_authority_emissions, &self.mint_liquidity_emissions, &token_2022_program_id());

        let token_accounts_ix = endcoin::CreateTokenAccountsInstruction::data(
            endcoin::CreateTokenAccountsInstructionData::new(),
        )
        .accounts(endcoin::CreateTokenAccountsInstructionAccounts::new(
            self.pool_account_a_emissions,
            self.pool_account_b_emissions,
            self.pool_authority_emissions,
            self.amm,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.authority,
        ))
        .instruction();
        let token_accounts_result =
            self.trident
                .process_transaction(&[token_accounts_ix], Some("CreateTokenAccounts"));
        assert!(
            token_accounts_result.is_success(),
            "expected CreateTokenAccounts success, got: {:?}\nlogs: {}",
            token_accounts_result.get_result(),
            token_accounts_result.logs()
        );
        let (mint_a, owner_a, _) =
            token_account_fields(&mut self.trident, &self.pool_account_a_emissions)
                .expect("pool_account_a should exist");
        assert_eq!(mint_a, self.mint_a_emissions);
        assert_eq!(owner_a, self.pool_authority_emissions);
        let (mint_b, owner_b, _) =
            token_account_fields(&mut self.trident, &self.pool_account_b_emissions)
                .expect("pool_account_b should exist");
        assert_eq!(mint_b, self.mint_b_emissions);
        assert_eq!(owner_b, self.pool_authority_emissions);

        // CreateRewardVault: first with wrong payer (UnauthorizedAdmin), then with correct payer.
        (self.reward_vault, _) = self.trident.find_program_address(
            &[
                self.pool_emissions.as_ref(),
                self.mint_a_emissions.as_ref(),
                self.mint_b_emissions.as_ref(),
                b"reward-vault".as_ref(),
            ],
            &endcoin::program_id(),
        );

        let wrong_payer = self.trident.random_pubkey();
        self.trident.airdrop(&wrong_payer, LAMPORTS_PER_SOL);
        let reward_vault_wrong_ix = endcoin::CreateRewardVaultInstruction::data(
            endcoin::CreateRewardVaultInstructionData::new(),
        )
        .accounts(endcoin::CreateRewardVaultInstructionAccounts::new(
            self.reward_vault,
            self.pool_emissions,
            self.amm,
            self.mint_a_emissions,
            self.mint_b_emissions,
            wrong_payer,
        ))
        .instruction();
        let reward_vault_wrong_result = self.trident.process_transaction(
            &[reward_vault_wrong_ix],
            Some("CreateRewardVaultUnauthorized"),
        );
        assert!(
            reward_vault_wrong_result.is_custom_error_with_code(AMM_ERROR_UNAUTHORIZED_ADMIN),
            "expected UnauthorizedAdmin({AMM_ERROR_UNAUTHORIZED_ADMIN}), got: {:?}\nlogs: {}",
            reward_vault_wrong_result.get_result(),
            reward_vault_wrong_result.logs()
        );

        let reward_vault_ix = endcoin::CreateRewardVaultInstruction::data(
            endcoin::CreateRewardVaultInstructionData::new(),
        )
        .accounts(endcoin::CreateRewardVaultInstructionAccounts::new(
            self.reward_vault,
            self.pool_emissions,
            self.amm,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.authority,
        ))
        .instruction();
        let reward_vault_result =
            self.trident
                .process_transaction(&[reward_vault_ix], Some("CreateRewardVault"));
        assert!(
            reward_vault_result.is_success(),
            "expected CreateRewardVault success, got: {:?}\nlogs: {}",
            reward_vault_result.get_result(),
            reward_vault_result.logs()
        );
        let reward_vault_state: RewardVault = self
            .trident
            .get_account_with_type(&self.reward_vault, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("RewardVault should deserialize after CreateRewardVault");
        assert_eq!(reward_vault_state.pool, self.pool_emissions);
        assert_eq!(reward_vault_state.mint_a, self.mint_a_emissions);
        assert_eq!(reward_vault_state.mint_b, self.mint_b_emissions);

        // CreateRewardTokenAccounts (also tests UnauthorizedAdmin path once).
        self.reward_account_a =
            ata_address(&self.reward_vault, &self.mint_a_emissions, &token_2022_program_id());
        self.reward_account_b =
            ata_address(&self.reward_vault, &self.mint_b_emissions, &token_2022_program_id());

        let reward_token_wrong_ix = endcoin::CreateRewardTokenAccountsInstruction::data(
            endcoin::CreateRewardTokenAccountsInstructionData::new(),
        )
        .accounts(endcoin::CreateRewardTokenAccountsInstructionAccounts::new(
            self.pool_emissions,
            self.amm,
            self.reward_account_a,
            self.reward_account_b,
            self.reward_vault,
            self.mint_a_emissions,
            self.mint_b_emissions,
            wrong_payer,
        ))
        .instruction();
        let reward_token_wrong_result = self.trident.process_transaction(
            &[reward_token_wrong_ix],
            Some("CreateRewardTokenAccountsUnauthorized"),
        );
        assert!(
            reward_token_wrong_result.is_custom_error_with_code(AMM_ERROR_UNAUTHORIZED_ADMIN),
            "expected UnauthorizedAdmin({AMM_ERROR_UNAUTHORIZED_ADMIN}), got: {:?}\nlogs: {}",
            reward_token_wrong_result.get_result(),
            reward_token_wrong_result.logs()
        );

        let reward_token_ix = endcoin::CreateRewardTokenAccountsInstruction::data(
            endcoin::CreateRewardTokenAccountsInstructionData::new(),
        )
        .accounts(endcoin::CreateRewardTokenAccountsInstructionAccounts::new(
            self.pool_emissions,
            self.amm,
            self.reward_account_a,
            self.reward_account_b,
            self.reward_vault,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.authority,
        ))
        .instruction();
        let reward_token_result = self.trident.process_transaction(
            &[reward_token_ix],
            Some("CreateRewardTokenAccounts"),
        );
        assert!(
            reward_token_result.is_success(),
            "expected CreateRewardTokenAccounts success, got: {:?}\nlogs: {}",
            reward_token_result.get_result(),
            reward_token_result.logs()
        );
        let (reward_mint_a, reward_owner_a, _) =
            token_account_fields(&mut self.trident, &self.reward_account_a)
                .expect("reward_account_a should exist");
        assert_eq!(reward_mint_a, self.mint_a_emissions);
        assert_eq!(reward_owner_a, self.reward_vault);
        let (reward_mint_b, reward_owner_b, _) =
            token_account_fields(&mut self.trident, &self.reward_account_b)
                .expect("reward_account_b should exist");
        assert_eq!(reward_mint_b, self.mint_b_emissions);
        assert_eq!(reward_owner_b, self.reward_vault);

        // Seed some rewards so ClaimReward can succeed.
        let seed_rewards_ix = endcoin::DepositRewardsInstruction::data(
            endcoin::DepositRewardsInstructionData::new(20.0),
        )
        .accounts(endcoin::DepositRewardsInstructionAccounts::new(
            self.reward_vault,
            self.pool_emissions,
            self.authority,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.reward_account_a,
            self.reward_account_b,
            self.mint_authority,
        ))
        .instruction();
        let seed_rewards_result =
            self.trident
                .process_transaction(&[seed_rewards_ix], Some("DepositRewards"));
        assert!(
            seed_rewards_result.is_success(),
            "expected initial DepositRewards success, got: {:?}\nlogs: {}",
            seed_rewards_result.get_result(),
            seed_rewards_result.logs()
        );

        // Setup a claimer for ClaimReward (ATAs created on-demand by instruction).
        self.claimer = self.trident.random_pubkey();
        self.trident.airdrop(&self.claimer, LAMPORTS_PER_SOL);
        self.claimer_account_a =
            ata_address(&self.claimer, &self.mint_a_emissions, &token_2022_program_id());
        self.claimer_account_b =
            ata_address(&self.claimer, &self.mint_b_emissions, &token_2022_program_id());

        // ---------------------------------------------------------------------
        // Swap setup (CreatePool + CreateTokenAccounts + mint balances)
        // ---------------------------------------------------------------------
        self.mint_a_swap = self.trident.random_pubkey();
        self.mint_b_swap = self.trident.random_pubkey();
        self.mint_liquidity_swap = self.trident.random_pubkey();

        let swap_mints_ixs = vec![
            system_instruction::create_account(
                &payer,
                &self.mint_a_swap,
                rent.minimum_balance(MINT_ACCOUNT_LEN as usize),
                MINT_ACCOUNT_LEN,
                &token_2022_program_id(),
            ),
            token2022_initialize_mint2_ix(self.mint_a_swap, payer, decimals),
            system_instruction::create_account(
                &payer,
                &self.mint_b_swap,
                rent.minimum_balance(MINT_ACCOUNT_LEN as usize),
                MINT_ACCOUNT_LEN,
                &token_2022_program_id(),
            ),
            token2022_initialize_mint2_ix(self.mint_b_swap, payer, decimals),
            system_instruction::create_account(
                &payer,
                &self.mint_liquidity_swap,
                rent.minimum_balance(MINT_ACCOUNT_LEN as usize),
                MINT_ACCOUNT_LEN,
                &token_2022_program_id(),
            ),
            token2022_initialize_mint2_ix(self.mint_liquidity_swap, payer, decimals),
        ];
        let swap_mints_result = self.trident.process_transaction(&swap_mints_ixs, None);
        assert!(
            swap_mints_result.is_success(),
            "expected swap mints setup success, got: {:?}\nlogs: {}",
            swap_mints_result.get_result(),
            swap_mints_result.logs()
        );

        (self.pool_swap, _) = self.trident.find_program_address(
            &[
                self.amm.as_ref(),
                self.mint_a_swap.as_ref(),
                self.mint_b_swap.as_ref(),
            ],
            &endcoin::program_id(),
        );
        (self.pool_authority_swap, _) = self.trident.find_program_address(
            &[
                self.amm.as_ref(),
                self.mint_a_swap.as_ref(),
                self.mint_b_swap.as_ref(),
                b"pool-authority".as_ref(),
            ],
            &endcoin::program_id(),
        );

        let pool_swap_ix =
            endcoin::CreatePoolInstruction::data(endcoin::CreatePoolInstructionData::new())
                .accounts(endcoin::CreatePoolInstructionAccounts::new(
                    self.amm,
                    self.pool_swap,
                    self.pool_authority_swap,
                    self.mint_liquidity_swap,
                    self.mint_a_swap,
                    self.mint_b_swap,
                    self.authority,
                ))
                .instruction();
        let pool_swap_result = self.trident.process_transaction(&[pool_swap_ix], Some("CreatePool"));
        assert!(
            pool_swap_result.is_success(),
            "expected CreatePool (swap) success, got: {:?}\nlogs: {}",
            pool_swap_result.get_result(),
            pool_swap_result.logs()
        );

        self.pool_account_a_swap =
            ata_address(&self.pool_authority_swap, &self.mint_a_swap, &token_2022_program_id());
        self.pool_account_b_swap =
            ata_address(&self.pool_authority_swap, &self.mint_b_swap, &token_2022_program_id());

        let pool_token_accounts_ix = endcoin::CreateTokenAccountsInstruction::data(
            endcoin::CreateTokenAccountsInstructionData::new(),
        )
        .accounts(endcoin::CreateTokenAccountsInstructionAccounts::new(
            self.pool_account_a_swap,
            self.pool_account_b_swap,
            self.pool_authority_swap,
            self.amm,
            self.mint_a_swap,
            self.mint_b_swap,
            self.authority,
        ))
        .instruction();
        let pool_token_accounts_result = self.trident.process_transaction(
            &[pool_token_accounts_ix],
            Some("CreateTokenAccounts"),
        );
        assert!(
            pool_token_accounts_result.is_success(),
            "expected CreateTokenAccounts (swap) success, got: {:?}\nlogs: {}",
            pool_token_accounts_result.get_result(),
            pool_token_accounts_result.logs()
        );

        self.trader = self.trident.random_pubkey();
        self.trident.airdrop(&self.trader, LAMPORTS_PER_SOL);
        self.trader_account_a =
            ata_address(&self.trader, &self.mint_a_swap, &token_2022_program_id());
        self.trader_account_b =
            ata_address(&self.trader, &self.mint_b_swap, &token_2022_program_id());

        let trader_ata_ixs = vec![
            create_ata_idempotent_ix(
                payer,
                self.trader_account_a,
                self.trader,
                self.mint_a_swap,
                token_2022_program_id(),
            ),
            create_ata_idempotent_ix(
                payer,
                self.trader_account_b,
                self.trader,
                self.mint_b_swap,
                token_2022_program_id(),
            ),
        ];
        let trader_ata_result = self.trident.process_transaction(&trader_ata_ixs, None);
        assert!(
            trader_ata_result.is_success(),
            "expected trader ATA setup success, got: {:?}\nlogs: {}",
            trader_ata_result.get_result(),
            trader_ata_result.logs()
        );

        // Seed balances: pool reserves + trader inventory.
        let seed_swap_balances_ixs = vec![
            token2022_mint_to_checked_ix(
                self.mint_a_swap,
                self.pool_account_a_swap,
                payer,
                1_000_000,
                decimals,
            ),
            token2022_mint_to_checked_ix(
                self.mint_b_swap,
                self.pool_account_b_swap,
                payer,
                1_000_000,
                decimals,
            ),
            token2022_mint_to_checked_ix(
                self.mint_a_swap,
                self.trader_account_a,
                payer,
                100_000,
                decimals,
            ),
            token2022_mint_to_checked_ix(
                self.mint_b_swap,
                self.trader_account_b,
                payer,
                100_000,
                decimals,
            ),
        ];
        let seed_swap_result = self.trident.process_transaction(&seed_swap_balances_ixs, None);
        assert!(
            seed_swap_result.is_success(),
            "expected seeding swap balances success, got: {:?}\nlogs: {}",
            seed_swap_result.get_result(),
            seed_swap_result.logs()
        );

        // Time account PDA for UpdateTimestamp.
        (self.time, _) =
            self.trident
                .find_program_address(&[b"time".as_ref()], &endcoin::program_id());
    }

    #[flow]
    fn flow_update_fee(&mut self) {
        // Test: UpdateFee enforces admin authority and updates fee on success.
        let before: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("AMM must exist before UpdateFee");
        assert!(before.created, "AMM must be created before UpdateFee");

        // Randomly choose whether to use the correct admin or an incorrect one.
        let use_correct_admin = self.trident.random_bool();
        let signer = if use_correct_admin {
            self.current_admin
        } else {
            let mut wrong = self.trident.random_pubkey();
            while wrong == self.current_admin {
                wrong = self.trident.random_pubkey();
            }
            self.trident.airdrop(&wrong, 1);
            wrong
        };

        let new_fee: u16 = self.trident.random_from_range(0u16..=10_000u16);
        let ix = endcoin::UpdateFeeInstruction::data(endcoin::UpdateFeeInstructionData::new(
            new_fee,
        ))
        .accounts(endcoin::UpdateFeeInstructionAccounts::new(self.amm, signer))
        .instruction();

        let result = self
            .trident
            .process_transaction(&[ix], Some("UpdateFee"));

        let after: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("AMM must exist after UpdateFee");

        if use_correct_admin {
            assert!(
                result.is_success(),
                "expected UpdateFee success, got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after.admin, before.admin);
            assert_eq!(after.fee, new_fee);
            self.current_fee = new_fee;
        } else {
            // If the AMM wasn't created, we'd expect NotCreated. Otherwise NotSigner.
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_NOT_SIGNER)
                    || result.is_custom_error_with_code(AMM_ERROR_NOT_CREATED),
                "expected NotSigner({AMM_ERROR_NOT_SIGNER}) or NotCreated({AMM_ERROR_NOT_CREATED}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after.admin, before.admin);
            assert_eq!(after.fee, before.fee);
        }
    }

    #[flow]
    fn flow_update_admin(&mut self) {
        // Test: UpdateAdmin enforces admin authority and updates admin on success.
        let before: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("AMM must exist before UpdateAdmin");
        assert!(before.created, "AMM must be created before UpdateAdmin");

        let use_correct_admin = self.trident.random_bool();
        let signer = if use_correct_admin {
            self.current_admin
        } else {
            let mut wrong = self.trident.random_pubkey();
            while wrong == self.current_admin {
                wrong = self.trident.random_pubkey();
            }
            self.trident.airdrop(&wrong, 1);
            wrong
        };

        let mut new_admin = self.trident.random_pubkey();
        while new_admin == before.admin {
            new_admin = self.trident.random_pubkey();
        }
        self.trident.airdrop(&new_admin, 1);

        let ix = endcoin::UpdateAdminInstruction::data(
            endcoin::UpdateAdminInstructionData::new(new_admin),
        )
        .accounts(endcoin::UpdateAdminInstructionAccounts::new(self.amm, signer))
        .instruction();

        let result = self
            .trident
            .process_transaction(&[ix], Some("UpdateAdmin"));

        let after: Amm = self
            .trident
            .get_account_with_type(&self.amm, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("AMM must exist after UpdateAdmin");

        if use_correct_admin {
            assert!(
                result.is_success(),
                "expected UpdateAdmin success, got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after.admin, new_admin);
            assert_eq!(after.fee, before.fee);
            self.current_admin = new_admin;
        } else {
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_NOT_SIGNER)
                    || result.is_custom_error_with_code(AMM_ERROR_NOT_CREATED),
                "expected NotSigner({AMM_ERROR_NOT_SIGNER}) or NotCreated({AMM_ERROR_NOT_CREATED}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after.admin, before.admin);
            assert_eq!(after.fee, before.fee);
        }
    }

    #[flow]
    fn flow_deposit_liquidity(&mut self) {
        // Targeted negative tests for temperature validation.
        if self.trident.random_from_range(0u8..=19u8) == 0 {
            let mean_temp = if self.trident.random_bool() {
                f64::NAN
            } else {
                36.0
            };

            let before_a = token_account_amount(&mut self.trident, &self.pool_account_a_emissions);
            let before_b = token_account_amount(&mut self.trident, &self.pool_account_b_emissions);
            let before_lp =
                token_account_amount(&mut self.trident, &self.depositor_liquidity_emissions);

            let ix = endcoin::DepositLiquidityInstruction::data(
                endcoin::DepositLiquidityInstructionData::new(mean_temp),
            )
            .accounts(endcoin::DepositLiquidityInstructionAccounts::new(
                self.pool_emissions,
                self.pool_authority_emissions,
                self.authority,
                self.mint_liquidity_emissions,
                self.mint_a_emissions,
                self.mint_b_emissions,
                self.pool_account_a_emissions,
                self.pool_account_b_emissions,
                self.depositor_liquidity_emissions,
                self.mint_authority,
            ))
            .instruction();

            let result = self.trident.process_transaction(&[ix], Some("DepositLiquidityInvalidTemp"));
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_INVALID_TEMPERATURE),
                "expected InvalidTemperature({AMM_ERROR_INVALID_TEMPERATURE}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );

            let after_a = token_account_amount(&mut self.trident, &self.pool_account_a_emissions);
            let after_b = token_account_amount(&mut self.trident, &self.pool_account_b_emissions);
            let after_lp =
                token_account_amount(&mut self.trident, &self.depositor_liquidity_emissions);
            assert_eq!(after_a, before_a, "unexpected A balance change on invalid temp");
            assert_eq!(after_b, before_b, "unexpected B balance change on invalid temp");
            assert_eq!(after_lp, before_lp, "unexpected LP balance change on invalid temp");
            return;
        }

        let mean_temp: f64 = self.trident.random_from_range(0.0f64..=35.0f64);

        let before_a = token_account_amount(&mut self.trident, &self.pool_account_a_emissions);
        let before_b = token_account_amount(&mut self.trident, &self.pool_account_b_emissions);
        let before_lp =
            token_account_amount(&mut self.trident, &self.depositor_liquidity_emissions);

        let ix = endcoin::DepositLiquidityInstruction::data(
            endcoin::DepositLiquidityInstructionData::new(mean_temp),
        )
        .accounts(endcoin::DepositLiquidityInstructionAccounts::new(
            self.pool_emissions,
            self.pool_authority_emissions,
            self.authority,
            self.mint_liquidity_emissions,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.pool_account_a_emissions,
            self.pool_account_b_emissions,
            self.depositor_liquidity_emissions,
            self.mint_authority,
        ))
        .instruction();

        let result = self
            .trident
            .process_transaction(&[ix], Some("DepositLiquidity"));

        let after_a = token_account_amount(&mut self.trident, &self.pool_account_a_emissions);
        let after_b = token_account_amount(&mut self.trident, &self.pool_account_b_emissions);
        let after_lp = token_account_amount(&mut self.trident, &self.depositor_liquidity_emissions);

        if result.is_success() {
            assert_token_account_mint_owner(
                &mut self.trident,
                &self.pool_account_a_emissions,
                self.mint_a_emissions,
                self.pool_authority_emissions,
            );
            assert_token_account_mint_owner(
                &mut self.trident,
                &self.pool_account_b_emissions,
                self.mint_b_emissions,
                self.pool_authority_emissions,
            );
            // May be created on first success.
            assert_token_account_mint_owner(
                &mut self.trident,
                &self.depositor_liquidity_emissions,
                self.mint_liquidity_emissions,
                self.pool_authority_emissions,
            );

            assert!(
                after_a > before_a && after_b > before_b && after_lp >= before_lp,
                "expected pool balances to increase (a,b) and lp to be non-decreasing; got before (a={before_a}, b={before_b}, lp={before_lp}) after (a={after_a}, b={after_b}, lp={after_lp})"
            );
        } else {
            // Emission curve rounds to 0 at extremes; allow DepositTooSmall / InvalidTemperature here.
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_DEPOSIT_TOO_SMALL)
                    || result.is_custom_error_with_code(AMM_ERROR_INVALID_TEMPERATURE),
                "expected DepositTooSmall({AMM_ERROR_DEPOSIT_TOO_SMALL}) or InvalidTemperature({AMM_ERROR_INVALID_TEMPERATURE}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
        }
    }

    #[flow]
    fn flow_deposit_rewards(&mut self) {
        // Targeted negative tests for temperature validation.
        if self.trident.random_from_range(0u8..=49u8) == 0 {
            let mean_temp = if self.trident.random_bool() {
                f64::INFINITY
            } else {
                -1.0
            };

            let before_a = token_account_amount(&mut self.trident, &self.reward_account_a);
            let before_b = token_account_amount(&mut self.trident, &self.reward_account_b);

            let ix = endcoin::DepositRewardsInstruction::data(
                endcoin::DepositRewardsInstructionData::new(mean_temp),
            )
            .accounts(endcoin::DepositRewardsInstructionAccounts::new(
                self.reward_vault,
                self.pool_emissions,
                self.authority,
                self.mint_a_emissions,
                self.mint_b_emissions,
                self.reward_account_a,
                self.reward_account_b,
                self.mint_authority,
            ))
            .instruction();

            let result = self.trident.process_transaction(&[ix], Some("DepositRewardsInvalidTemp"));
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_INVALID_TEMPERATURE),
                "expected InvalidTemperature({AMM_ERROR_INVALID_TEMPERATURE}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );

            let after_a = token_account_amount(&mut self.trident, &self.reward_account_a);
            let after_b = token_account_amount(&mut self.trident, &self.reward_account_b);
            assert_eq!(after_a, before_a);
            assert_eq!(after_b, before_b);
            return;
        }

        // Keep this in a band where reward-share emissions are typically non-zero.
        let mean_temp: f64 = self.trident.random_from_range(5.0f64..=32.0f64);

        let before_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let before_b = token_account_amount(&mut self.trident, &self.reward_account_b);

        let ix = endcoin::DepositRewardsInstruction::data(endcoin::DepositRewardsInstructionData::new(
            mean_temp,
        ))
        .accounts(endcoin::DepositRewardsInstructionAccounts::new(
            self.reward_vault,
            self.pool_emissions,
            self.authority,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.reward_account_a,
            self.reward_account_b,
            self.mint_authority,
        ))
        .instruction();

        let result = self.trident.process_transaction(&[ix], Some("DepositRewards"));

        let after_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let after_b = token_account_amount(&mut self.trident, &self.reward_account_b);

        if result.is_success() {
            assert!(
                after_a > before_a && after_b > before_b,
                "expected reward balances to increase; got before (a={before_a}, b={before_b}) after (a={after_a}, b={after_b})"
            );
        } else {
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_DEPOSIT_TOO_SMALL)
                    || result.is_custom_error_with_code(AMM_ERROR_INVALID_TEMPERATURE),
                "expected DepositTooSmall({AMM_ERROR_DEPOSIT_TOO_SMALL}) or InvalidTemperature({AMM_ERROR_INVALID_TEMPERATURE}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
        }
    }

    #[flow]
    fn flow_claim_reward(&mut self) {
        // Top up rewards if needed so we can hit the success path frequently.
        let mut balance_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let mut balance_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        if balance_a == 0 || balance_b == 0 {
            let topup_ix = endcoin::DepositRewardsInstruction::data(
                endcoin::DepositRewardsInstructionData::new(20.0),
            )
            .accounts(endcoin::DepositRewardsInstructionAccounts::new(
                self.reward_vault,
                self.pool_emissions,
                self.authority,
                self.mint_a_emissions,
                self.mint_b_emissions,
                self.reward_account_a,
                self.reward_account_b,
                self.mint_authority,
            ))
            .instruction();
            let topup_result = self
                .trident
                .process_transaction(&[topup_ix], Some("DepositRewards"));
            assert!(
                topup_result.is_success()
                    || topup_result.is_custom_error_with_code(AMM_ERROR_DEPOSIT_TOO_SMALL),
                "unexpected deposit topup result: {:?}\nlogs: {}",
                topup_result.get_result(),
                topup_result.logs()
            );

            balance_a = token_account_amount(&mut self.trident, &self.reward_account_a);
            balance_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        }

        let use_correct_claimer = self.trident.random_from_range(0u8..=9u8) != 0;
        let claimer_arg = if use_correct_claimer {
            self.claimer
        } else {
            let mut wrong = self.trident.random_pubkey();
            while wrong == self.claimer {
                wrong = self.trident.random_pubkey();
            }
            wrong
        };

        // Targeted negative: insufficient reward by intentionally exceeding balance sometimes.
        let force_insufficient = use_correct_claimer
            && self.trident.random_from_range(0u8..=19u8) == 0
            && balance_a > 0
            && balance_b > 0;

        let amount_a = if force_insufficient {
            balance_a + 1
        } else if balance_a == 0 {
            1
        } else {
            let max = balance_a.min(10_000);
            self.trident.random_from_range(1u64..=max)
        };
        let amount_b = if force_insufficient {
            balance_b + 1
        } else if balance_b == 0 {
            1
        } else {
            let max = balance_b.min(10_000);
            self.trident.random_from_range(1u64..=max)
        };

        let before_reward_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let before_reward_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        let before_claimer_a = token_account_amount(&mut self.trident, &self.claimer_account_a);
        let before_claimer_b = token_account_amount(&mut self.trident, &self.claimer_account_b);
        let before_total_a =
            (before_reward_a as u128).saturating_add(before_claimer_a as u128);
        let before_total_b =
            (before_reward_b as u128).saturating_add(before_claimer_b as u128);

        let ix = endcoin::ClaimRewardInstruction::data(endcoin::ClaimRewardInstructionData::new(
            claimer_arg,
            amount_a,
            amount_b,
        ))
        .accounts(endcoin::ClaimRewardInstructionAccounts::new(
            self.pool_emissions,
            self.claimer,
            self.claimer_account_a,
            self.claimer_account_b,
            self.mint_a_emissions,
            self.mint_b_emissions,
            self.reward_vault,
            self.reward_account_a,
            self.reward_account_b,
        ))
        .instruction();

        let result = self.trident.process_transaction(&[ix], Some("ClaimReward"));

        let after_reward_a = token_account_amount(&mut self.trident, &self.reward_account_a);
        let after_reward_b = token_account_amount(&mut self.trident, &self.reward_account_b);
        let after_claimer_a = token_account_amount(&mut self.trident, &self.claimer_account_a);
        let after_claimer_b = token_account_amount(&mut self.trident, &self.claimer_account_b);
        let after_total_a = (after_reward_a as u128).saturating_add(after_claimer_a as u128);
        let after_total_b = (after_reward_b as u128).saturating_add(after_claimer_b as u128);

        if use_correct_claimer && before_reward_a >= amount_a && before_reward_b >= amount_b {
            assert!(
                result.is_success(),
                "expected ClaimReward success, got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_token_account_mint_owner(
                &mut self.trident,
                &self.reward_account_a,
                self.mint_a_emissions,
                self.reward_vault,
            );
            assert_token_account_mint_owner(
                &mut self.trident,
                &self.reward_account_b,
                self.mint_b_emissions,
                self.reward_vault,
            );
            assert_eq!(after_reward_a, before_reward_a - amount_a);
            assert_eq!(after_reward_b, before_reward_b - amount_b);
            assert_eq!(after_claimer_a, before_claimer_a + amount_a);
            assert_eq!(after_claimer_b, before_claimer_b + amount_b);
            assert_eq!(after_total_a, before_total_a, "A conservation failed");
            assert_eq!(after_total_b, before_total_b, "B conservation failed");
        } else if !use_correct_claimer {
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_UNAUTHORIZED_CLAIMER),
                "expected UnauthorizedClaimer({AMM_ERROR_UNAUTHORIZED_CLAIMER}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after_reward_a, before_reward_a);
            assert_eq!(after_reward_b, before_reward_b);
            assert_eq!(after_claimer_a, before_claimer_a);
            assert_eq!(after_claimer_b, before_claimer_b);
        } else {
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_INSUFFICIENT_REWARD),
                "expected InsufficientReward({AMM_ERROR_INSUFFICIENT_REWARD}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after_reward_a, before_reward_a);
            assert_eq!(after_reward_b, before_reward_b);
            assert_eq!(after_claimer_a, before_claimer_a);
            assert_eq!(after_claimer_b, before_claimer_b);
        }
    }

    #[flow]
    fn flow_swap(&mut self) {
        let decimals: u8 = 6;
        // Ensure reserves and trader inventory remain non-zero.
        let pool_a = token_account_amount(&mut self.trident, &self.pool_account_a_swap);
        let pool_b = token_account_amount(&mut self.trident, &self.pool_account_b_swap);
        let trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);
        if pool_a == 0 || pool_b == 0 || trader_a == 0 || trader_b == 0 {
            let topup_ixs = vec![
                token2022_mint_to_checked_ix(self.mint_a_swap, self.pool_account_a_swap, self.authority, 1_000_000, decimals),
                token2022_mint_to_checked_ix(self.mint_b_swap, self.pool_account_b_swap, self.authority, 1_000_000, decimals),
                token2022_mint_to_checked_ix(self.mint_a_swap, self.trader_account_a, self.authority, 100_000, decimals),
                token2022_mint_to_checked_ix(self.mint_b_swap, self.trader_account_b, self.authority, 100_000, decimals),
            ];
            let topup_result = self.trident.process_transaction(&topup_ixs, None);
            assert!(
                topup_result.is_success(),
                "expected swap topup success, got: {:?}\nlogs: {}",
                topup_result.get_result(),
                topup_result.logs()
            );
        }

        // Snapshot balances for conservation checks.
        let before_pool_a = token_account_amount(&mut self.trident, &self.pool_account_a_swap);
        let before_pool_b = token_account_amount(&mut self.trident, &self.pool_account_b_swap);
        let before_trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let before_trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);
        let before_total_a = (before_pool_a as u128).saturating_add(before_trader_a as u128);
        let before_total_b = (before_pool_b as u128).saturating_add(before_trader_b as u128);

        let swap_a = self.trident.random_bool();
        let input_balance = if swap_a { before_trader_a } else { before_trader_b };
        let input_amount = self
            .trident
            .random_from_range(0u64..=input_balance.min(10_000));

        // Targeted negative: enforce OutputTooSmall by setting min_output above reserves.
        if self.trident.random_from_range(0u8..=19u8) == 0 && input_amount > 0 {
            let reserve_out = if swap_a { before_pool_b } else { before_pool_a };
            let impossible_min = reserve_out.saturating_add(1);

            let ix_fail = endcoin::SwapInstruction::data(endcoin::SwapInstructionData::new(
                swap_a,
                input_amount,
                impossible_min,
            ))
            .accounts(endcoin::SwapInstructionAccounts::new(
                self.amm,
                self.pool_authority_swap,
                self.trader,
                self.mint_a_swap,
                self.mint_b_swap,
                self.pool_swap,
                self.pool_account_a_swap,
                self.pool_account_b_swap,
                self.sst,
                self.trader_account_a,
                self.trader_account_b,
                self.authority,
            ))
            .instruction();

            let result =
                self.trident
                    .process_transaction(&[ix_fail], Some("SwapMinOutputTooHigh"));
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_OUTPUT_TOO_SMALL),
                "expected OutputTooSmall({AMM_ERROR_OUTPUT_TOO_SMALL}), got: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );

            let after_pool_a = token_account_amount(&mut self.trident, &self.pool_account_a_swap);
            let after_pool_b = token_account_amount(&mut self.trident, &self.pool_account_b_swap);
            let after_trader_a =
                token_account_amount(&mut self.trident, &self.trader_account_a);
            let after_trader_b =
                token_account_amount(&mut self.trident, &self.trader_account_b);
            assert_eq!(after_pool_a, before_pool_a);
            assert_eq!(after_pool_b, before_pool_b);
            assert_eq!(after_trader_a, before_trader_a);
            assert_eq!(after_trader_b, before_trader_b);
            return;
        }

        let ix = endcoin::SwapInstruction::data(endcoin::SwapInstructionData::new(
            swap_a,
            input_amount,
            0,
        ))
        .accounts(endcoin::SwapInstructionAccounts::new(
            self.amm,
            self.pool_authority_swap,
            self.trader,
            self.mint_a_swap,
            self.mint_b_swap,
            self.pool_swap,
            self.pool_account_a_swap,
            self.pool_account_b_swap,
            self.sst,
            self.trader_account_a,
            self.trader_account_b,
            self.authority,
        ))
        .instruction();

        let result = self.trident.process_transaction(&[ix], Some("Swap"));

        let after_pool_a = token_account_amount(&mut self.trident, &self.pool_account_a_swap);
        let after_pool_b = token_account_amount(&mut self.trident, &self.pool_account_b_swap);
        let after_trader_a = token_account_amount(&mut self.trident, &self.trader_account_a);
        let after_trader_b = token_account_amount(&mut self.trident, &self.trader_account_b);
        let after_total_a = (after_pool_a as u128).saturating_add(after_trader_a as u128);
        let after_total_b = (after_pool_b as u128).saturating_add(after_trader_b as u128);

        if result.is_success() {
            // Expected exact input transfer (no fee is taken by the token program; fee stays in pool via pricing).
            if swap_a {
                assert_eq!(after_trader_a, before_trader_a - input_amount);
                assert_eq!(after_pool_a, before_pool_a + input_amount);
                let out = after_trader_b.saturating_sub(before_trader_b);
                assert!(out > 0, "expected some output on success");
                assert_eq!(after_pool_b, before_pool_b - out);
            } else {
                assert_eq!(after_trader_b, before_trader_b - input_amount);
                assert_eq!(after_pool_b, before_pool_b + input_amount);
                let out = after_trader_a.saturating_sub(before_trader_a);
                assert!(out > 0, "expected some output on success");
                assert_eq!(after_pool_a, before_pool_a - out);
            }

            // Conservation: all movement is between trader and pool accounts.
            assert_eq!(after_total_a, before_total_a, "A conservation failed");
            assert_eq!(after_total_b, before_total_b, "B conservation failed");
        } else {
            // Allow common swap precondition failures and ensure no balance changes.
            assert!(
                result.is_custom_error_with_code(AMM_ERROR_INPUT_AMOUNT_TOO_SMALL)
                    || result.is_custom_error_with_code(AMM_ERROR_INSUFFICIENT_BALANCE)
                    || result.is_custom_error_with_code(AMM_ERROR_INSUFFICIENT_LIQUIDITY)
                    || result.is_custom_error_with_code(AMM_ERROR_INVALID_TEMPERATURE)
                    || result.is_custom_error_with_code(AMM_ERROR_OUTPUT_TOO_SMALL),
                "unexpected Swap error: {:?}\nlogs: {}",
                result.get_result(),
                result.logs()
            );
            assert_eq!(after_pool_a, before_pool_a);
            assert_eq!(after_pool_b, before_pool_b);
            assert_eq!(after_trader_a, before_trader_a);
            assert_eq!(after_trader_b, before_trader_b);
        }
    }

    #[flow]
    fn flow_pull_feed(&mut self) {
        let ix = endcoin::PullFeedInstruction::data(endcoin::PullFeedInstructionData::new())
            .accounts(endcoin::PullFeedInstructionAccounts::new(self.feed))
            .instruction();

        let result = self.trident.process_transaction(&[ix], Some("PullFeed"));
        assert!(
            result.is_success(),
            "expected PullFeed success, got: {:?}\nlogs: {}",
            result.get_result(),
            result.logs()
        );
    }

    #[flow]
    fn flow_update_timestamp(&mut self) {
        let ix = endcoin::UpdateTimestampInstruction::data(
            endcoin::UpdateTimestampInstructionData::new(),
        )
        .accounts(endcoin::UpdateTimestampInstructionAccounts::new(
            self.time,
            self.authority,
        ))
        .instruction();

        let result = self
            .trident
            .process_transaction(&[ix], Some("UpdateTimestamp"));
        assert!(
            result.is_success(),
            "expected UpdateTimestamp success, got: {:?}\nlogs: {}",
            result.get_result(),
            result.logs()
        );

        let time_state: Time = self
            .trident
            .get_account_with_type(&self.time, ANCHOR_DISCRIMINATOR_BYTES)
            .expect("Time should deserialize after UpdateTimestamp");
        assert!(time_state.created);
    }

    #[end]
    fn end(&mut self) {
        // Perform any cleanup here, this method will be executed
        // at the end of each iteration
    }
}

fn main() {
    FuzzTest::fuzz(1000, 100);
}
