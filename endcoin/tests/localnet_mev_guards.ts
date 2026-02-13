import { expect } from "chai";
import * as anchor from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  createAccount,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { PublicKey, Transaction } from "@solana/web3.js";
import { ensureLocalnetInitialized, LocalnetContext } from "../scripts/localnet/common";

const AMM_ERROR_UNAUTHORIZED_CLAIMER = 6009;
const AMM_ERROR_INVALID_POOL_ACCOUNT = 6019;
const AMM_ERROR_PARAMETER_RECENTLY_UPDATED = 6024;
const AMM_ERROR_PRICE_IMPACT_TOO_HIGH = 6025;

function parseCustomErrorCode(err: any): number | null {
  const byAnchor = err?.error?.errorCode?.number;
  if (typeof byAnchor === "number") return byAnchor;

  const chunks: string[] = [];
  if (typeof err?.message === "string") chunks.push(err.message);
  if (typeof err?.toString === "function") chunks.push(String(err.toString()));
  if (Array.isArray(err?.logs)) chunks.push(err.logs.join("\n"));
  if (Array.isArray(err?.error?.logs)) chunks.push(err.error.logs.join("\n"));
  try {
    chunks.push(JSON.stringify(err));
  } catch {
    // ignore non-serializable errors
  }

  const text = chunks.join("\n");
  const anchorNumber = text.match(/Error Number:\s*(\d+)/i);
  if (anchorNumber) return Number(anchorNumber[1]);

  const customHex = text.match(/custom program error: 0x([0-9a-f]+)/i);
  if (customHex) return parseInt(customHex[1], 16);

  return null;
}

async function expectCustomError(promise: Promise<unknown>, code: number) {
  try {
    await promise;
    expect.fail(`Expected custom error ${code}`);
  } catch (err: any) {
    const parsed = parseCustomErrorCode(err);
    expect(parsed, `Expected custom error ${code}, got ${String(err?.message || err)}`).to.equal(code);
  }
}

function deriveTimePda(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("time")], programId)[0];
}

async function advanceSlot(ctx: LocalnetContext) {
  const time = deriveTimePda(ctx.program.programId);
  await ctx.program.methods
    .updateTimestamp()
    .accountsStrict({
      time,
      payer: ctx.payer.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
}

function swapAccounts(ctx: LocalnetContext) {
  const traderA = getAssociatedTokenAddressSync(
    ctx.mintA.publicKey,
    ctx.payer.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID
  );
  const traderB = getAssociatedTokenAddressSync(
    ctx.mintB.publicKey,
    ctx.payer.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  return {
    traderA,
    traderB,
    accounts: {
      amm: ctx.addresses.amm,
      poolAuthority: ctx.addresses.poolAuthority,
      trader: ctx.payer.publicKey,
      mintA: ctx.mintA.publicKey,
      mintB: ctx.mintB.publicKey,
      pool: ctx.addresses.pool,
      poolAccountA: ctx.addresses.poolAccountA,
      poolAccountB: ctx.addresses.poolAccountB,
      sst: ctx.addresses.sst,
      traderAccountA: traderA,
      traderAccountB: traderB,
      payer: ctx.payer.publicKey,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    },
  };
}

async function bootstrapLiquidityAndRewards(ctx: LocalnetContext) {
  const { program, payer, mintA, mintB, mintLiquidity, addresses } = ctx;

  await program.methods
    .pullFeed()
    .accountsStrict({
      amm: addresses.amm,
      sst: addresses.sst,
      feed: addresses.oracleFeed,
    })
    .rpc();

  await advanceSlot(ctx);

  await program.methods
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
      systemProgram: anchor.web3.SystemProgram.programId,
      mintAuthority: addresses.mintAuthority,
    })
    .rpc();

  await program.methods
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
      systemProgram: anchor.web3.SystemProgram.programId,
      mintAuthority: addresses.mintAuthority,
    })
    .rpc();

  await program.methods
    .updateRewardWhitelist(payer.publicKey)
    .accountsStrict({
      rewardVault: addresses.rewardVault,
      pool: addresses.pool,
      amm: addresses.amm,
      mintA: mintA.publicKey,
      mintB: mintB.publicKey,
      admin: payer.publicKey,
    })
    .rpc();

  const rewardBalA = await ctx.provider.connection.getTokenAccountBalance(addresses.rewardAccountA);
  const rewardBalB = await ctx.provider.connection.getTokenAccountBalance(addresses.rewardAccountB);
  const claimA = new anchor.BN(rewardBalA.value.amount).div(new anchor.BN(5));
  const claimB = new anchor.BN(rewardBalB.value.amount).div(new anchor.BN(5));

  if (claimA.gt(new anchor.BN(0)) && claimB.gt(new anchor.BN(0))) {
    await program.methods
      .claimReward(claimA, claimB)
      .accountsStrict({
        pool: addresses.pool,
        claimer: payer.publicKey,
        toMintAAccount: getAssociatedTokenAddressSync(
          mintA.publicKey,
          payer.publicKey,
          false,
          TOKEN_2022_PROGRAM_ID
        ),
        toMintBAccount: getAssociatedTokenAddressSync(
          mintB.publicKey,
          payer.publicKey,
          false,
          TOKEN_2022_PROGRAM_ID
        ),
        mintA: mintA.publicKey,
        mintB: mintB.publicKey,
        rewardVault: addresses.rewardVault,
        whitelistAuthority: payer.publicKey,
        rewardAccountA: addresses.rewardAccountA,
        rewardAccountB: addresses.rewardAccountB,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
  }

  await advanceSlot(ctx);
}

describe("localnet MEV guards (surfpool)", () => {
  it("blocks unauthorized reward claims via whitelist authority", async () => {
    const ctx = await ensureLocalnetInitialized();
    await bootstrapLiquidityAndRewards(ctx);

    const outsider = anchor.web3.Keypair.generate().publicKey;
    await ctx.program.methods
      .updateRewardWhitelist(outsider)
      .accountsStrict({
        rewardVault: ctx.addresses.rewardVault,
        pool: ctx.addresses.pool,
        amm: ctx.addresses.amm,
        mintA: ctx.mintA.publicKey,
        mintB: ctx.mintB.publicKey,
        admin: ctx.payer.publicKey,
      })
      .rpc();

    await expectCustomError(
      ctx.program.methods
        .claimReward(new anchor.BN(1), new anchor.BN(1))
        .accountsStrict({
          pool: ctx.addresses.pool,
          claimer: ctx.payer.publicKey,
          toMintAAccount: getAssociatedTokenAddressSync(
            ctx.mintA.publicKey,
            ctx.payer.publicKey,
            false,
            TOKEN_2022_PROGRAM_ID
          ),
          toMintBAccount: getAssociatedTokenAddressSync(
            ctx.mintB.publicKey,
            ctx.payer.publicKey,
            false,
            TOKEN_2022_PROGRAM_ID
          ),
          mintA: ctx.mintA.publicKey,
          mintB: ctx.mintB.publicKey,
          rewardVault: ctx.addresses.rewardVault,
          whitelistAuthority: ctx.payer.publicKey,
          rewardAccountA: ctx.addresses.rewardAccountA,
          rewardAccountB: ctx.addresses.rewardAccountB,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc(),
      AMM_ERROR_UNAUTHORIZED_CLAIMER
    );

    // Restore default whitelist signer for other tests.
    await ctx.program.methods
      .updateRewardWhitelist(ctx.payer.publicKey)
      .accountsStrict({
        rewardVault: ctx.addresses.rewardVault,
        pool: ctx.addresses.pool,
        amm: ctx.addresses.amm,
        mintA: ctx.mintA.publicKey,
        mintB: ctx.mintB.publicKey,
        admin: ctx.payer.publicKey,
      })
      .rpc();
  });

  it("blocks swaps when non-canonical reserve accounts are provided", async () => {
    const ctx = await ensureLocalnetInitialized();
    await bootstrapLiquidityAndRewards(ctx);

    const spoofedPoolAccountA = await createAccount(
      ctx.provider.connection,
      ctx.payer,
      ctx.mintA.publicKey,
      ctx.addresses.poolAuthority,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    const { accounts } = swapAccounts(ctx);
    const traderBalanceA = await ctx.provider.connection.getTokenAccountBalance(accounts.traderAccountA);
    const input = new anchor.BN(traderBalanceA.value.amount).div(new anchor.BN(20));
    expect(input.gt(new anchor.BN(0))).to.equal(true);

    await expectCustomError(
      ctx.program.methods
        .swap(true, input, new anchor.BN(1))
        .accountsStrict({
          ...accounts,
          poolAccountA: spoofedPoolAccountA,
        })
        .rpc(),
      AMM_ERROR_INVALID_POOL_ACCOUNT
    );
  });

  it("blocks same-slot fee update + swap attempts and still swaps after cooldown", async () => {
    const ctx = await ensureLocalnetInitialized();
    await bootstrapLiquidityAndRewards(ctx);

    const { accounts } = swapAccounts(ctx);
    const traderBalanceA = await ctx.provider.connection.getTokenAccountBalance(accounts.traderAccountA);
    const input = new anchor.BN(traderBalanceA.value.amount).div(new anchor.BN(30));
    expect(input.gt(new anchor.BN(0))).to.equal(true);

    const updateIx = await ctx.program.methods
      .updateFee(600)
      .accountsStrict({
        amm: ctx.addresses.amm,
        admin: ctx.payer.publicKey,
      })
      .instruction();

    const swapIx = await ctx.program.methods
      .swap(true, input, new anchor.BN(1))
      .accountsStrict(accounts)
      .instruction();

    await expectCustomError(
      ctx.provider.sendAndConfirm(new Transaction().add(updateIx, swapIx), []),
      AMM_ERROR_PARAMETER_RECENTLY_UPDATED
    );

    await advanceSlot(ctx);

    await ctx.program.methods
      .swap(true, input, new anchor.BN(1))
      .accountsStrict(accounts)
      .rpc({ skipPreflight: true });
  });

  it("blocks same-slot oracle refresh + swap attempts", async () => {
    const ctx = await ensureLocalnetInitialized();
    await bootstrapLiquidityAndRewards(ctx);

    const { accounts } = swapAccounts(ctx);
    const traderBalanceA = await ctx.provider.connection.getTokenAccountBalance(accounts.traderAccountA);
    const input = new anchor.BN(traderBalanceA.value.amount).div(new anchor.BN(40));
    expect(input.gt(new anchor.BN(0))).to.equal(true);

    const pullIx = await ctx.program.methods
      .pullFeed()
      .accountsStrict({
        amm: ctx.addresses.amm,
        sst: ctx.addresses.sst,
        feed: ctx.addresses.oracleFeed,
      })
      .instruction();

    const swapIx = await ctx.program.methods
      .swap(true, input, new anchor.BN(1))
      .accountsStrict(accounts)
      .instruction();

    await expectCustomError(
      ctx.provider.sendAndConfirm(new Transaction().add(pullIx, swapIx), []),
      AMM_ERROR_PARAMETER_RECENTLY_UPDATED
    );
  });

  it("blocks outsized price-impact swaps", async () => {
    const ctx = await ensureLocalnetInitialized();
    await bootstrapLiquidityAndRewards(ctx);

    // Build a larger trader inventory by repeatedly minting rewards then claiming them.
    for (let i = 0; i < 8; i += 1) {
      await ctx.program.methods
        .depositRewards()
        .accountsStrict({
          amm: ctx.addresses.amm,
          rewardVault: ctx.addresses.rewardVault,
          pool: ctx.addresses.pool,
          sst: ctx.addresses.sst,
          payer: ctx.payer.publicKey,
          mintA: ctx.mintA.publicKey,
          mintB: ctx.mintB.publicKey,
          rewardAccountA: ctx.addresses.rewardAccountA,
          rewardAccountB: ctx.addresses.rewardAccountB,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
          mintAuthority: ctx.addresses.mintAuthority,
        })
        .rpc();
    }

    const rewardBalA = await ctx.provider.connection.getTokenAccountBalance(ctx.addresses.rewardAccountA);
    const rewardBalB = await ctx.provider.connection.getTokenAccountBalance(ctx.addresses.rewardAccountB);
    const claimA = new anchor.BN(rewardBalA.value.amount).mul(new anchor.BN(9)).div(new anchor.BN(10));
    const claimB = new anchor.BN(rewardBalB.value.amount).mul(new anchor.BN(9)).div(new anchor.BN(10));

    await ctx.program.methods
      .claimReward(claimA, claimB)
      .accountsStrict({
        pool: ctx.addresses.pool,
        claimer: ctx.payer.publicKey,
        toMintAAccount: getAssociatedTokenAddressSync(
          ctx.mintA.publicKey,
          ctx.payer.publicKey,
          false,
          TOKEN_2022_PROGRAM_ID
        ),
        toMintBAccount: getAssociatedTokenAddressSync(
          ctx.mintB.publicKey,
          ctx.payer.publicKey,
          false,
          TOKEN_2022_PROGRAM_ID
        ),
        mintA: ctx.mintA.publicKey,
        mintB: ctx.mintB.publicKey,
        rewardVault: ctx.addresses.rewardVault,
        whitelistAuthority: ctx.payer.publicKey,
        rewardAccountA: ctx.addresses.rewardAccountA,
        rewardAccountB: ctx.addresses.rewardAccountB,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    await advanceSlot(ctx);

    const { accounts } = swapAccounts(ctx);
    const poolReserveA = await ctx.provider.connection.getTokenAccountBalance(ctx.addresses.poolAccountA);
    const traderBalanceA = await ctx.provider.connection.getTokenAccountBalance(accounts.traderAccountA);

    const reserve = new anchor.BN(poolReserveA.value.amount);
    const trader = new anchor.BN(traderBalanceA.value.amount);
    const input = reserve.lt(trader) ? reserve : trader;

    expect(input.gt(new anchor.BN(0))).to.equal(true);

    await expectCustomError(
      ctx.program.methods
        .swap(true, input, new anchor.BN(1))
        .accountsStrict(accounts)
        .rpc({ skipPreflight: true }),
      AMM_ERROR_PRICE_IMPACT_TOO_HIGH
    );
  });
});
