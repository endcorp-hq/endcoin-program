import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createMint,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { SystemProgram, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { Endcoin } from "../target/types/endcoin";
import { readFileSync } from "fs";
import path from "path";

// Utility to load a keypair from a JSON file that could be either a raw array or an object
function loadKeypair(fileRelative: string, objectProp?: string): anchor.web3.Keypair {
  const absolutePath = path.resolve(__dirname, fileRelative);
  const raw = JSON.parse(readFileSync(absolutePath, "utf8"));
  const secret: number[] = Array.isArray(raw) ? raw : (objectProp ? raw[objectProp] : undefined);
  if (!secret) {
    throw new Error(`Invalid keypair file format at ${absolutePath}`);
  }
  return anchor.web3.Keypair.fromSecretKey(Uint8Array.from(secret));
}

async function accountExists(connection: anchor.web3.Connection, pubkey: PublicKey) {
  const info = await connection.getAccountInfo(pubkey);
  return !!info;
}

async function assertMintIsToken2022(
  connection: anchor.web3.Connection,
  mintPubkey: PublicKey,
  label: string
) {
  const info = await connection.getAccountInfo(mintPubkey);
  if (!info) return; // will be created below as Token-2022
  if (!info.owner.equals(TOKEN_2022_PROGRAM_ID)) {
    const ownerStr = info.owner.toBase58();
    throw new Error(
      `${label} mint ${mintPubkey.toBase58()} exists but is owned by ${ownerStr}. ` +
      `This program requires Token-2022 mints (${TOKEN_2022_PROGRAM_ID.toBase58()}). ` +
      `Replace the keypair with a fresh one that has never been initialized, or delete/rekey the existing account.`
    );
  }
}

async function ensureMintExists(
  connection: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mintKeypair: anchor.web3.Keypair,
  decimals = 6
) {
  const info = await connection.getAccountInfo(mintKeypair.publicKey);
  if (info) return; // already exists
  await createMint(
    connection,
    payer,
    payer.publicKey,
    payer.publicKey,
    decimals,
    mintKeypair,
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
}

async function maybeAirdrop(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  minLamports: number
) {
  const bal = await connection.getBalance(pubkey);
  if (bal >= minLamports) return;
  try {
    const sig = await connection.requestAirdrop(pubkey, Math.max(minLamports - bal, 1 * LAMPORTS_PER_SOL));
    const latest = await connection.getLatestBlockhash();
    await connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");
  } catch (_) {
    // ignore on devnet/mainnet where airdrop may fail
  }
}

async function run() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const connection = provider.connection;
  const program = anchor.workspace.Endcoin as Program<Endcoin>;

  // Load admin (AMM admin) and mint keypairs
  const admin = loadKeypair("./keys/wba-wallet.json");
  const mintAKeypair = loadKeypair("./keys/endXt7KhrE3vSTsAg4wdnwZ3J5cKauosC6HJjmyT2Ee.json", "endcoin_mint");
  const mintBKeypair = loadKeypair("./keys/gaiajL3kBXQwr9iW5ytinFkcECCatQ5fTMj4ZpjgF9s.json", "gaiacoin_mint");
  const mintLiquidityKeypair = loadKeypair("./keys/pLsmEBaphhZ2bLaqQb9kqRFnigB5G9hGEENRMgYA9Nf.json", "liquidity_mint");

  // Ensure payer and admin have SOL for rent
  await maybeAirdrop(connection, provider.wallet.publicKey, 2 * LAMPORTS_PER_SOL);
  await maybeAirdrop(connection, admin.publicKey, 2 * LAMPORTS_PER_SOL);

  // Deterministically ensure all mints are Token-2022 (fail fast if legacy)
  await assertMintIsToken2022(connection, mintAKeypair.publicKey, "Endcoin (mintA)");
  await assertMintIsToken2022(connection, mintBKeypair.publicKey, "Gaiacoin (mintB)");
  await assertMintIsToken2022(connection, mintLiquidityKeypair.publicKey, "Liquidity (LP)");

  // // Create the three Token-2022 mints if missing
  // await ensureMintExists(connection, provider.wallet.payer, mintAKeypair, 6);
  // await ensureMintExists(connection, provider.wallet.payer, mintBKeypair, 6);
  // await ensureMintExists(connection, provider.wallet.payer, mintLiquidityKeypair, 6);

  // Derive PDAs
  const ammKey = PublicKey.findProgramAddressSync([Buffer.from("amm")], program.programId)[0];
  const poolKey = PublicKey.findProgramAddressSync(
    [ammKey.toBuffer(), mintAKeypair.publicKey.toBuffer(), mintBKeypair.publicKey.toBuffer()],
    program.programId
  )[0];
  const poolAuthority = PublicKey.findProgramAddressSync(
    [
      ammKey.toBuffer(),
      mintAKeypair.publicKey.toBuffer(),
      mintBKeypair.publicKey.toBuffer(),
      Buffer.from("pool-authority"),
    ],
    program.programId
  )[0];
  const rewardVault = PublicKey.findProgramAddressSync(
    [poolKey.toBuffer(), mintAKeypair.publicKey.toBuffer(), mintBKeypair.publicKey.toBuffer(), Buffer.from("reward-vault")],
    program.programId
  )[0];

  // Pre-compute ATA addresses required by program instructions (Token-2022)
  const poolAccountA = getAssociatedTokenAddressSync(
    mintAKeypair.publicKey,
    poolAuthority,
    true,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  const poolAccountB = getAssociatedTokenAddressSync(
    mintBKeypair.publicKey,
    poolAuthority,
    true,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  // 1) Create AMM if missing
  const ammAlreadyExists = await accountExists(connection, ammKey);
  if (!ammAlreadyExists) {
    const fee = 500; // 5.00%
    await program.methods
      .createAmm(fee)
      .accountsStrict({
        amm: ammKey,
        admin: admin.publicKey,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc({ skipPreflight: false });
  } else {
    console.log("AMM already exists:", ammKey.toBase58());
  }

  // Read AMM to confirm admin matches provided admin
  const ammAccount = await program.account.amm.fetch(ammKey).catch(() => null);
  if (!ammAccount) {
    throw new Error(
      `AMM account ${ammKey.toBase58()} exists but is not owned by this program or has wrong account type.`
    );
  }
  if (!ammAccount.created) {
    throw new Error(`AMM account ${ammKey.toBase58()} exists but is not initialized (created=false).`);
  }
  if (!ammAccount.admin.equals(admin.publicKey)) {
    throw new Error(
      `AMM admin mismatch. On-chain: ${ammAccount.admin.toBase58()} vs provided admin ${admin.publicKey.toBase58()}.`
    );
  }

  // 2) Create Pool if missing
  const poolExists = await accountExists(connection, poolKey);
  if (!poolExists) {
    await program.methods
      .createPool()
      .accountsStrict({
        amm: ammKey,
        pool: poolKey,
        poolAuthority: poolAuthority,
        mintLiquidity: mintLiquidityKeypair.publicKey,
        mintA: mintAKeypair.publicKey,
        mintB: mintBKeypair.publicKey,
        payer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([provider.wallet.payer])
      .rpc({ skipPreflight: false });
  } else {
    console.log("Pool already exists:", poolKey.toBase58());
  }

  // 3) Create Pool Token Accounts (ATAs) if missing
  const poolAExists = await accountExists(connection, poolAccountA);
  const poolBExists = await accountExists(connection, poolAccountB);
  if (!poolAExists || !poolBExists) {
    await program.methods
      .createTokenAccounts()
      .accountsStrict({
        poolAccountA,
        poolAccountB,
        poolAuthority,
        amm: ammKey,
        mintA: mintAKeypair.publicKey,
        mintB: mintBKeypair.publicKey,
        payer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([provider.wallet.payer])
      .rpc({ skipPreflight: false });
  } else {
    console.log("Pool ATAs already exist:", poolAccountA.toBase58(), poolAccountB.toBase58());
  }

  // 4) Create Reward Vault if missing (payer must be AMM admin)
  const rewardVaultExists = await accountExists(connection, rewardVault);
  if (!rewardVaultExists) {
    await program.methods
      .createRewardVault()
      .accountsStrict({
        rewardVault,
        pool: poolKey,
        amm: ammKey,
        mintA: mintAKeypair.publicKey,
        mintB: mintBKeypair.publicKey,
        payer: admin.publicKey,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([admin])
      .rpc({ skipPreflight: false });
  } else {
    console.log("RewardVault already exists:", rewardVault.toBase58());
  }

  // 5) Create Reward Token Accounts (ATAs owned by reward_vault) if missing
  const rewardAccountA = getAssociatedTokenAddressSync(
    mintAKeypair.publicKey,
    rewardVault,
    true,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  const rewardAccountB = getAssociatedTokenAddressSync(
    mintBKeypair.publicKey,
    rewardVault,
    true,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  const rewardAExists = await accountExists(connection, rewardAccountA);
  const rewardBExists = await accountExists(connection, rewardAccountB);
  if (!rewardAExists || !rewardBExists) {
    await program.methods
      .createRewardTokenAccounts()
      .accountsStrict({
        pool: poolKey,
        amm: ammKey,
        rewardAccountA,
        rewardAccountB,
        rewardVault,
        mintA: mintAKeypair.publicKey,
        mintB: mintBKeypair.publicKey,
        payer: admin.publicKey,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([admin])
      .rpc({ skipPreflight: false });
  } else {
    console.log("Reward ATAs already exist:", rewardAccountA.toBase58(), rewardAccountB.toBase58());
  }

  // Log summary
  console.log("Deployment complete:");
  console.log("AMM:", ammKey.toBase58());
  console.log("Pool:", poolKey.toBase58());
  console.log("PoolAuthority:", poolAuthority.toBase58());
  console.log("Pool ATA A:", poolAccountA.toBase58());
  console.log("Pool ATA B:", poolAccountB.toBase58());
  console.log("RewardVault:", rewardVault.toBase58());
  console.log("Reward ATA A:", rewardAccountA.toBase58());
  console.log("Reward ATA B:", rewardAccountB.toBase58());
}

run().catch((err) => {
  console.error("deploy-all failed:", err?.message || err?.toString?.() || err);
  process.exit(1);
});
