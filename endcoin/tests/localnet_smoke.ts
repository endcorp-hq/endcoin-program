import { expect } from "chai";
import * as anchor from "@coral-xyz/anchor";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { ensureLocalnetInitialized } from "../scripts/localnet/common";

describe("localnet smoke (surfpool)", () => {
  it("initializes required accounts (idempotent)", async () => {
    const ctx = await ensureLocalnetInitialized();
    const amm = await ctx.program.account.amm.fetch(ctx.addresses.amm);
    expect(amm.admin.toBase58()).to.equal(ctx.payer.publicKey.toBase58());
  });

  it("emits → claims → swaps", async () => {
    const ctx = await ensureLocalnetInitialized();
    const { provider, program, payer, mintA, mintB, mintLiquidity, addresses } = ctx;

    const meanTemp = 21;

    await program.methods
      .depositLiquidity(meanTemp)
      .accountsStrict({
        pool: addresses.pool,
        poolAuthority: addresses.poolAuthority,
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
        systemProgram: anchor.web3.SystemProgram.programId,
        mintAuthority: addresses.mintAuthority,
      })
      .rpc();

    await program.methods
      .depositRewards(meanTemp)
      .accountsStrict({
        rewardVault: addresses.rewardVault,
        pool: addresses.pool,
        payer: payer.publicKey,
        mintA: mintA.publicKey,
        mintB: mintB.publicKey,
        rewardAccountA: addresses.rewardAccountA,
        rewardAccountB: addresses.rewardAccountB,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        mintAuthority: addresses.mintAuthority,
      })
      .rpc();

    const rewardBalA = await provider.connection.getTokenAccountBalance(addresses.rewardAccountA);
    const rewardBalB = await provider.connection.getTokenAccountBalance(addresses.rewardAccountB);
    const rewardAmountA = new anchor.BN(rewardBalA.value.amount);
    const rewardAmountB = new anchor.BN(rewardBalB.value.amount);
    expect(rewardAmountA.gt(new anchor.BN(0))).to.equal(true);
    expect(rewardAmountB.gt(new anchor.BN(0))).to.equal(true);

    const claimA = rewardAmountA.div(new anchor.BN(10));
    const claimB = rewardAmountB.div(new anchor.BN(10));

    await program.methods
      .claimReward(payer.publicKey, claimA, claimB)
      .accountsStrict({
        pool: addresses.pool,
        claimer: payer.publicKey,
        toMintAAccount: getAssociatedTokenAddressSync(mintA.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID),
        toMintBAccount: getAssociatedTokenAddressSync(mintB.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID),
        mintA: mintA.publicKey,
        mintB: mintB.publicKey,
        rewardVault: addresses.rewardVault,
        rewardAccountA: addresses.rewardAccountA,
        rewardAccountB: addresses.rewardAccountB,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const traderA = getAssociatedTokenAddressSync(mintA.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID);
    const traderB = getAssociatedTokenAddressSync(mintB.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID);

    const beforeA = await provider.connection.getTokenAccountBalance(traderA);
    const beforeB = await provider.connection.getTokenAccountBalance(traderB);

    const beforeAmountA = new anchor.BN(beforeA.value.amount);
    const beforeAmountB = new anchor.BN(beforeB.value.amount);
    const input = beforeAmountA.div(new anchor.BN(10));
    expect(input.gt(new anchor.BN(0))).to.equal(true);

    await program.methods
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
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc({ skipPreflight: true });

    const afterA = await provider.connection.getTokenAccountBalance(traderA);
    const afterB = await provider.connection.getTokenAccountBalance(traderB);

    const afterAmountA = new anchor.BN(afterA.value.amount);
    const afterAmountB = new anchor.BN(afterB.value.amount);
    expect(afterAmountA.lt(beforeAmountA)).to.equal(true);
    expect(afterAmountB.gt(beforeAmountB)).to.equal(true);
  });
});
