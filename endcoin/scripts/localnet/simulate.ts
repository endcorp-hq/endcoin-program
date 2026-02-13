import * as anchor from "@coral-xyz/anchor";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { SystemProgram } from "@solana/web3.js";
import { ensureLocalnetInitialized } from "./common";

async function main() {
  const ctx = await ensureLocalnetInitialized();
  const { provider, program, payer, mintA, mintB, mintLiquidity, addresses } = ctx;

  await program.methods
    .pullFeed()
    .accountsStrict({
      amm: addresses.amm,
      sst: addresses.sst,
      feed: addresses.oracleFeed,
    })
    .rpc();

  const sigLiquidity = await program.methods
    .depositLiquidity()
    .accountsStrict({
      amm: addresses.amm,
      pool: addresses.pool,
      poolAuthority: addresses.poolAuthority,
      sst: addresses.sst,
      payer: payer.publicKey,
      mintLiquidity: mintLiquidity.publicKey,
      mintA: mintA.publicKey,
      mintB: mintB.publicKey,
      poolAccountA: addresses.poolAccountA,
      poolAccountB: addresses.poolAccountB,
      depositorAccountLiquidity: getAssociatedTokenAddressSync(
        mintLiquidity.publicKey,
        addresses.poolAuthority,
        true,
        TOKEN_2022_PROGRAM_ID
      ),
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      mintAuthority: addresses.mintAuthority,
    })
    .rpc();
  console.log("depositLiquidity:", sigLiquidity);

  const sigRewards = await program.methods
    .depositRewards()
    .accountsStrict({
      amm: addresses.amm,
      rewardVault: addresses.rewardVault,
      pool: addresses.pool,
      sst: addresses.sst,
      payer: payer.publicKey,
      mintA: mintA.publicKey,
      mintB: mintB.publicKey,
      rewardAccountA: addresses.rewardAccountA,
      rewardAccountB: addresses.rewardAccountB,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      mintAuthority: addresses.mintAuthority,
    })
    .rpc();
  console.log("depositRewards:", sigRewards);

  const rewardBalA = await provider.connection.getTokenAccountBalance(addresses.rewardAccountA);
  const rewardBalB = await provider.connection.getTokenAccountBalance(addresses.rewardAccountB);
  const claimA = new anchor.BN(rewardBalA.value.amount).div(new anchor.BN(10));
  const claimB = new anchor.BN(rewardBalB.value.amount).div(new anchor.BN(10));

  const sigClaim = await program.methods
    .claimReward(claimA, claimB)
    .accountsStrict({
      pool: addresses.pool,
      claimer: payer.publicKey,
      toMintAAccount: getAssociatedTokenAddressSync(mintA.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID),
      toMintBAccount: getAssociatedTokenAddressSync(mintB.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID),
      mintA: mintA.publicKey,
      mintB: mintB.publicKey,
      rewardVault: addresses.rewardVault,
      whitelistAuthority: payer.publicKey,
      rewardAccountA: addresses.rewardAccountA,
      rewardAccountB: addresses.rewardAccountB,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log("claimReward:", sigClaim);

  const traderA = getAssociatedTokenAddressSync(mintA.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const traderB = getAssociatedTokenAddressSync(mintB.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const balA = await provider.connection.getTokenAccountBalance(traderA);
  const input = new anchor.BN(balA.value.amount).div(new anchor.BN(10));

  const sigSwap = await program.methods
    .swap(true, input, new anchor.BN(1))
    .accountsStrict({
      amm: addresses.amm,
      poolAuthority: addresses.poolAuthority,
      trader: payer.publicKey,
      mintA: mintA.publicKey,
      mintB: mintB.publicKey,
      pool: addresses.pool,
      poolAccountA: addresses.poolAccountA,
      poolAccountB: addresses.poolAccountB,
      sst: addresses.sst,
      traderAccountA: traderA,
      traderAccountB: traderB,
      payer: payer.publicKey,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc({ skipPreflight: true });
  console.log("swap A->B:", sigSwap);
}

main().catch((err) => {
  console.error(err?.message || err);
  process.exit(1);
});
