use anchor_lang::error_code;

#[error_code]
pub enum AmmError {
    #[msg("Invalid fee value")]
    InvalidFee,

    #[msg("Invalid mint for the pool")]
    InvalidMint,

    #[msg("Depositing too little liquidity")]
    DepositTooSmall,

    #[msg("Output is below the minimum expected")]
    OutputTooSmall,

    #[msg("Invariant does not hold")]
    InvariantViolated,

    #[msg("AMM Already Created")]
    AlreadyCreated,

    #[msg("Unauthorized Admin account used.")]
    UnauthorizedAdmin,

    #[msg("The AMM has not been created, cannot update.")]
    NotCreated,

    #[msg("Admin is not a signer")]
    NotSigner,

    #[msg("This account is not authorized to claim the reward.")]
    UnauthorizedClaimer,
    #[msg("Not enough tokens in the reward account.")]
    InsufficientReward,

    #[msg("Bump Error")]
    BumpError,

    #[msg("Overflow Error")]
    Overflow,

    #[msg("Invalid Fee BPS")]
    InvalidFeeBps,

    #[msg("Arithmetic Overflow")]
    ArithmeticOverflow,

    #[msg("Swap input amount is too small")]
    InputAmountTooSmall,

    #[msg("Trader balance is insufficient for requested input")]
    InsufficientBalance,

    #[msg("Pool reserves are empty for this swap")]
    InsufficientLiquidity,

    #[msg("Invalid or non-finite temperature reading")]
    InvalidTemperature,
}

#[error_code]
pub enum SstError {
    #[msg("SST Already Initialized")]
    AlreadyInitialized,
}

#[error_code]
pub enum FeedData {
    #[msg("Invalid Feed Data")]
    InvalidFeedData,
}

#[error_code]
pub enum MetadataError {
    #[msg("Metadata Invalid")]
    InvalidMetadata,
}

#[error_code]
#[derive(Eq, PartialEq)]
pub enum SwitchboardClientError {
    #[msg("Not a valid Switchboard account")]
    InvalidSwitchboardAccount,
    #[msg("Switchboard feed has not been updated in 5 minutes")]
    StaleFeed,
    #[msg("Switchboard feed exceeded provided confidence interval")]
    ConfidenceIntervalExceeded,
    #[msg("History buffer mismatch")]
    InvalidHistoryBuffer,
}
