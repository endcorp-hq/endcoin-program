import * as anchor from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { PublicKey, SystemProgram, Transaction, TransactionInstruction } from "@solana/web3.js";
import bs58 from "bs58";
import { ensureLocalnetInitialized, ensureToken2022Ata } from "./common";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import path from "path";

type TraderStateV1 = {
  version: 1;
  traders: number[][];
};

type Stats = {
  swaps: number;
  claims: number;
  liquidityDeposits: number;
  rewardDeposits: number;
  failures: number;
  latenciesMs: number[];
  computeUnits: number[];
  fairnessAbsError: number[];
  temperatureSamples: Array<{ temperature: number; outputPerInput: number }>;
};

const LOCALNET_DIR = path.resolve(__dirname, "../../.localnet");
const TRADERS_PATH = path.join(LOCALNET_DIR, "traders.json");
const INDEX_PATH = path.join(LOCALNET_DIR, "index-accounts.json");
const MEMO_PROGRAM_ID = new PublicKey("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

function ensureLocalnetDir() {
  if (!existsSync(LOCALNET_DIR)) mkdirSync(LOCALNET_DIR, { recursive: true });
}

function readJson<T>(filePath: string): T {
  return JSON.parse(readFileSync(filePath, "utf8")) as T;
}

function writeJson(filePath: string, data: unknown) {
  writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n", "utf8");
}

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.max(0, Math.floor((p / 100) * sorted.length)));
  return sorted[idx];
}

function mean(values: number[]): number {
  if (values.length === 0) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function clamp(x: number, min: number, max: number) {
  return Math.min(max, Math.max(min, x));
}

const DEATH_TEMP_C = 35;
const END_RATE = 1.125;
const GAIA_RATE = 0.75;

function temperatureWeights(temp: number): { weightEnd: number; weightGaia: number } {
  const t = clamp(temp, 0, DEATH_TEMP_C);
  const eVal = Math.exp((END_RATE * (DEATH_TEMP_C - t)) - 1.0);
  const gVal = Math.exp((GAIA_RATE * t) - 1.0);
  const total = eVal + gVal;
  return { weightEnd: eVal / total, weightGaia: gVal / total };
}

function applyFee(amount: anchor.BN, feeBps: number): anchor.BN {
  const fee = amount.mul(new anchor.BN(feeBps)).div(new anchor.BN(10_000));
  return amount.sub(fee);
}

function bnToSafeNumber(x: anchor.BN): number | null {
  const max = new anchor.BN(Number.MAX_SAFE_INTEGER.toString());
  if (x.gt(max)) return null;
  return Number(x.toString());
}

function expectedWeightedSwapOutput(params: {
  inputAmount: anchor.BN;
  reserveIn: anchor.BN;
  reserveOut: anchor.BN;
  weightIn: number;
  weightOut: number;
  feeBps: number;
}): number | null {
  const taxed = applyFee(params.inputAmount, params.feeBps);
  const reserveInNum = bnToSafeNumber(params.reserveIn);
  const reserveOutNum = bnToSafeNumber(params.reserveOut);
  const taxedNum = bnToSafeNumber(taxed);
  if (reserveInNum == null || reserveOutNum == null || taxedNum == null) return null;
  if (reserveInNum <= 0 || reserveOutNum <= 0 || taxedNum <= 0) return null;
  const base = reserveInNum / (reserveInNum + taxedNum);
  const power = Math.pow(base, params.weightIn / params.weightOut);
  const out = reserveOutNum * (1 - power);
  return Math.floor(out);
}

function logsContain(logs: string[] | undefined, needles: string[]): boolean {
  if (!logs || logs.length === 0) return false;
  const hay = logs.join("\n");
  return needles.some((n) => hay.includes(n));
}

function temperatureAt(i: number, low: number, high: number, mode: string): number {
  if (mode === "square") return i % 2 === 0 ? low : high;
  if (mode === "ramp") {
    const t = (i % 200) / 199;
    return low + (high - low) * t;
  }
  // default: sine
  const mid = (low + high) / 2;
  const amp = (high - low) / 2;
  return mid + amp * Math.sin(i / 10);
}

function sleepMs(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parseSwapLog(logs: string[]): { input: anchor.BN; output: anchor.BN; net: anchor.BN } | null {
  for (const line of logs) {
    const m = line.match(/Swap (A->B|B->A): input (\d+), net (\d+), output (\d+)/);
    if (!m) continue;
    return {
      input: new anchor.BN(m[2]),
      net: new anchor.BN(m[3]),
      output: new anchor.BN(m[4]),
    };
  }
  return null;
}

class TxFailedError extends Error {
  signature?: string;
  logs?: string[];
  constructor(label: string, message: string, signature?: string, logs?: string[]) {
    super(`${label} failed: ${message}`);
    this.signature = signature;
    this.logs = logs;
  }
}

function memoIx(text: string): TransactionInstruction {
  return new TransactionInstruction({
    programId: MEMO_PROGRAM_ID,
    keys: [],
    data: Buffer.from(text, "utf8"),
  });
}

async function sendWithLogs(params: {
  provider: anchor.AnchorProvider;
  tx: Transaction;
  extraSigners?: anchor.web3.Signer[];
  label: string;
  skipPreflight: boolean;
  maxRetries?: number;
  verbose: boolean;
}): Promise<string> {
  const { provider, tx, extraSigners = [], label, skipPreflight, maxRetries, verbose } = params;
  let signature: string | null = null;
  try {
    tx.feePayer = tx.feePayer ?? provider.wallet.publicKey;
    const latest = await provider.connection.getLatestBlockhash("confirmed");
    tx.recentBlockhash = latest.blockhash;
    for (const signer of extraSigners) {
      tx.partialSign(signer);
    }
    const signed = await provider.wallet.signTransaction(tx);
    signature = bs58.encode(signed.signature ?? new Uint8Array());
    const raw = signed.serialize();
    await provider.connection.sendRawTransaction(raw, {
      skipPreflight,
      preflightCommitment: "confirmed",
      maxRetries,
    });
    const confirmation = await provider.connection.confirmTransaction(
      { signature, ...latest },
      "confirmed"
    );
    if (confirmation.value.err) {
      const failedTx = await provider.connection.getTransaction(signature, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const logs = failedTx?.meta?.logMessages ?? [];
      if (logs.length && verbose) console.error(`${label} failed logs:`, logs);
      throw new TxFailedError(
        label,
        JSON.stringify(confirmation.value.err),
        signature,
        logs.length ? logs : undefined
      );
    }
    return signature;
  } catch (err: any) {
    const msg = err?.message || err?.toString?.() || String(err);
    // Common under heavy load if we accidentally re-send an identical tx/signature.
    if (typeof msg === "string" && msg.includes("already been processed")) {
      if (verbose) console.warn(`${label}: already processed (treating as success)`);
      return signature ?? (err?.signature as string) ?? "";
    }

    if (verbose) {
      console.error(`${label} failed:`, msg);
    }
    throw err;
  }
}

async function main() {
  const ITERATIONS = Number(process.env.ITERATIONS ?? "2000");
  const TRADERS = Number(process.env.TRADERS ?? "25");
  const SWAP_AMOUNT = new anchor.BN(process.env.SWAP_AMOUNT ?? "50000"); // base units (decimals=6 => 0.05 token)
  const CLAIM_AMOUNT = new anchor.BN(process.env.CLAIM_AMOUNT ?? "25000");
  const DEPOSIT_LIQ_EVERY = Number(process.env.DEPOSIT_LIQ_EVERY ?? "200");
  const DEPOSIT_REWARDS_EVERY = Number(process.env.DEPOSIT_REWARDS_EVERY ?? "25");
  const CLAIM_EVERY = Number(process.env.CLAIM_EVERY ?? "0");
  let TEMP_UPDATE_EVERY = Number(process.env.TEMP_UPDATE_EVERY ?? "10");
  const TEMP_LOW = Number(process.env.TEMP_LOW ?? "5");
  const TEMP_HIGH = Number(process.env.TEMP_HIGH ?? "30");
  const TEMP_MODE = process.env.TEMP_MODE ?? "sine"; // sine|square|ramp
  const SAMPLE_EVERY = Number(process.env.SAMPLE_EVERY ?? "50");
  const EMISSION_TEMP = Number(process.env.EMISSION_TEMP ?? "21");
  const SKIP_PREFLIGHT = (process.env.SKIP_PREFLIGHT ?? "1") !== "0";
  const VERBOSE = (process.env.VERBOSE ?? "0") === "1";
  const MAX_RETRIES = process.env.MAX_RETRIES ? Number(process.env.MAX_RETRIES) : undefined;
  const SEED_TRADERS = (process.env.SEED_TRADERS ?? "1") !== "0";
  const INITIAL_A = process.env.INITIAL_A
    ? new anchor.BN(process.env.INITIAL_A)
    : SWAP_AMOUNT.mul(new anchor.BN(200));
  const INITIAL_B = process.env.INITIAL_B
    ? new anchor.BN(process.env.INITIAL_B)
    : SWAP_AMOUNT.mul(new anchor.BN(200));
  const SWAP_MODE = (process.env.SWAP_MODE ?? "ab").toLowerCase(); // ab|ba|alternate|random
  const THROTTLE_MS = Number(process.env.THROTTLE_MS ?? "0");

  const ctx = await ensureLocalnetInitialized();
  const { provider, program, payer, mintA, mintB, mintLiquidity, addresses } = ctx;

  ensureLocalnetDir();

  const supportsTempUpdate = program.idl.instructions.some(
    (ix) => ix.name === "update_sst_temperature" || ix.name === "updateSstTemperature"
  );
  if (!supportsTempUpdate) {
    TEMP_UPDATE_EVERY = 0;
  }

  // Create (or reuse) trader keypairs (just for signatures; no SOL required if we pre-create ATAs).
  let traderState: TraderStateV1;
  if (existsSync(TRADERS_PATH)) {
    traderState = readJson<TraderStateV1>(TRADERS_PATH);
    if (traderState.version !== 1) throw new Error(`Unsupported traders state version: ${(traderState as any).version}`);
  } else {
    const traders = Array.from({ length: TRADERS }, () => Array.from(anchor.web3.Keypair.generate().secretKey));
    traderState = { version: 1, traders };
    writeJson(TRADERS_PATH, traderState);
  }

  const traders = traderState.traders.slice(0, TRADERS).map((sk) => anchor.web3.Keypair.fromSecretKey(Uint8Array.from(sk)));
  const traderBalancesA = new Map<string, anchor.BN>();
  const traderBalancesB = new Map<string, anchor.BN>();

  // Pre-create trader token accounts (paid by provider wallet so traders don't need SOL).
  for (const trader of traders) {
    await ensureToken2022Ata({
      connection: provider.connection,
      payer,
      mint: mintA.publicKey,
      owner: trader.publicKey,
    });
    await ensureToken2022Ata({
      connection: provider.connection,
      payer,
      mint: mintB.publicKey,
      owner: trader.publicKey,
    });
  }

  if (SEED_TRADERS) {
    for (let i = 0; i < traders.length; i++) {
      const trader = traders[i];
      const traderA = getAssociatedTokenAddressSync(mintA.publicKey, trader.publicKey, false, TOKEN_2022_PROGRAM_ID);
      const traderB = getAssociatedTokenAddressSync(mintB.publicKey, trader.publicKey, false, TOKEN_2022_PROGRAM_ID);
      try {
        const ix = await (program.methods as any)
          .claimReward(trader.publicKey, INITIAL_A, INITIAL_B)
          .accountsStrict({
            pool: addresses.pool,
            claimer: trader.publicKey,
            toMintAAccount: traderA,
            toMintBAccount: traderB,
            mintA: mintA.publicKey,
            mintB: mintB.publicKey,
            rewardVault: addresses.rewardVault,
            rewardAccountA: addresses.rewardAccountA,
            rewardAccountB: addresses.rewardAccountB,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .instruction();
        const tx = new Transaction().add(
          memoIx(`loadtest:seedTrader:${i}:${trader.publicKey.toBase58()}`),
          ix
        );
        await sendWithLogs({
          provider,
          tx,
          extraSigners: [trader],
          label: `seed trader ${i}`,
          skipPreflight: SKIP_PREFLIGHT,
          maxRetries: MAX_RETRIES,
          verbose: VERBOSE,
        });
        traderBalancesA.set(trader.publicKey.toBase58(), INITIAL_A);
        traderBalancesB.set(trader.publicKey.toBase58(), INITIAL_B);
      } catch (err: any) {
        if (
          err instanceof TxFailedError &&
          logsContain(err.logs, ["InsufficientReward", "Not enough tokens in the reward account"])
        ) {
          const depositIx = await (program.methods as any)
            .depositRewards(EMISSION_TEMP)
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
              systemProgram: SystemProgram.programId,
              mintAuthority: addresses.mintAuthority,
            })
            .instruction();
          const depositTx = new Transaction().add(memoIx(`loadtest:seed:rewards:refill:${i}`), depositIx);
          await sendWithLogs({
            provider,
            tx: depositTx,
            label: `refill rewards for seeding`,
            skipPreflight: SKIP_PREFLIGHT,
            maxRetries: MAX_RETRIES,
            verbose: VERBOSE,
          });
        }
        if (VERBOSE) {
          console.error(`seed trader ${i} failed:`, err?.message || err);
        }
        // Don't block the whole run; traders can still get funded later via CLAIM_EVERY logic.
      }
    }
  }

  // Write a convenient "accounts to index" list for Surfpool Studio.
  const indexAccounts = {
    programId: program.programId.toBase58(),
    amm: addresses.amm.toBase58(),
    pool: addresses.pool.toBase58(),
    sst: addresses.sst.toBase58(),
    poolAccountA: addresses.poolAccountA.toBase58(),
    poolAccountB: addresses.poolAccountB.toBase58(),
    rewardVault: addresses.rewardVault.toBase58(),
    rewardAccountA: addresses.rewardAccountA.toBase58(),
    rewardAccountB: addresses.rewardAccountB.toBase58(),
    traders: traders.map((t) => ({
      trader: t.publicKey.toBase58(),
      traderAccountA: getAssociatedTokenAddressSync(mintA.publicKey, t.publicKey, false, TOKEN_2022_PROGRAM_ID).toBase58(),
      traderAccountB: getAssociatedTokenAddressSync(mintB.publicKey, t.publicKey, false, TOKEN_2022_PROGRAM_ID).toBase58(),
    })),
  };
  writeJson(INDEX_PATH, indexAccounts);

  const stats: Stats = {
    swaps: 0,
    claims: 0,
    liquidityDeposits: 0,
    rewardDeposits: 0,
    failures: 0,
    latenciesMs: [],
    computeUnits: [],
    fairnessAbsError: [],
    temperatureSamples: [],
  };

  // Seed liquidity + rewards up-front.
  for (let i = 0; i < 3; i++) {
    const ix = await (program.methods as any)
      .depositLiquidity(EMISSION_TEMP)
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
        systemProgram: SystemProgram.programId,
        mintAuthority: addresses.mintAuthority,
      })
      .instruction();
    const tx = new Transaction().add(memoIx(`loadtest:seed:liq:${i}`), ix);
    await sendWithLogs({
      provider,
      tx,
      label: `seed depositLiquidity ${i}`,
      skipPreflight: SKIP_PREFLIGHT,
      maxRetries: MAX_RETRIES,
      verbose: VERBOSE,
    });
    stats.liquidityDeposits++;
  }

  for (let i = 0; i < 5; i++) {
    const ix = await (program.methods as any)
      .depositRewards(EMISSION_TEMP)
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
        systemProgram: SystemProgram.programId,
        mintAuthority: addresses.mintAuthority,
      })
      .instruction();
    const tx = new Transaction().add(memoIx(`loadtest:seed:rewards:${i}`), ix);
    await sendWithLogs({
      provider,
      tx,
      label: `seed depositRewards ${i}`,
      skipPreflight: SKIP_PREFLIGHT,
      maxRetries: MAX_RETRIES,
      verbose: VERBOSE,
    });
    stats.rewardDeposits++;
  }

  const startAll = Date.now();
  let currentTemperature = 21;
  const ammAccount: any = await (program as any).account.amm.fetch(addresses.amm);
  const feeBps: number = ammAccount.fee;

  for (let i = 1; i <= ITERATIONS; i++) {
    const trader = traders[i % traders.length];
    const traderKey = trader.publicKey.toBase58();
    const swapA =
      SWAP_MODE === "ab"
        ? true
        : SWAP_MODE === "ba"
          ? false
          : SWAP_MODE === "alternate"
            ? i % 2 === 0
            : Math.random() < 0.5;

    try {
      if (TEMP_UPDATE_EVERY > 0 && i % TEMP_UPDATE_EVERY === 0) {
        const nextTemp = clamp(temperatureAt(i, TEMP_LOW, TEMP_HIGH, TEMP_MODE), 0, 35);
        const ix = await (program.methods as any)
          .updateSstTemperature(nextTemp)
          .accountsStrict({
            amm: addresses.amm,
            admin: payer.publicKey,
            sst: addresses.sst,
          })
          .instruction();
        const tx = new Transaction().add(memoIx(`loadtest:temp:${i}:${nextTemp.toFixed(4)}`), ix);
        await sendWithLogs({
          provider,
          tx,
          label: `updateSstTemperature iter ${i}`,
          skipPreflight: SKIP_PREFLIGHT,
          maxRetries: MAX_RETRIES,
          verbose: VERBOSE,
        });
        currentTemperature = nextTemp;
      }

      if (DEPOSIT_LIQ_EVERY > 0 && i % DEPOSIT_LIQ_EVERY === 0) {
        const ix = await (program.methods as any)
          .depositLiquidity(EMISSION_TEMP)
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
            systemProgram: SystemProgram.programId,
            mintAuthority: addresses.mintAuthority,
          })
          .instruction();
        const tx = new Transaction().add(memoIx(`loadtest:liq:${i}`), ix);
        await sendWithLogs({
          provider,
          tx,
          label: `depositLiquidity iter ${i}`,
          skipPreflight: SKIP_PREFLIGHT,
          maxRetries: MAX_RETRIES,
          verbose: VERBOSE,
        });
        stats.liquidityDeposits++;
      }

      if (DEPOSIT_REWARDS_EVERY > 0 && i % DEPOSIT_REWARDS_EVERY === 0) {
        const ix = await (program.methods as any)
          .depositRewards(EMISSION_TEMP)
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
            systemProgram: SystemProgram.programId,
            mintAuthority: addresses.mintAuthority,
          })
          .instruction();
        const tx = new Transaction().add(memoIx(`loadtest:rewards:${i}`), ix);
        await sendWithLogs({
          provider,
          tx,
          label: `depositRewards iter ${i}`,
          skipPreflight: SKIP_PREFLIGHT,
          maxRetries: MAX_RETRIES,
          verbose: VERBOSE,
        });
        stats.rewardDeposits++;
      }

      if (CLAIM_EVERY > 0 && i % CLAIM_EVERY === 0) {
        const traderA = getAssociatedTokenAddressSync(mintA.publicKey, trader.publicKey, false, TOKEN_2022_PROGRAM_ID);
        const traderB = getAssociatedTokenAddressSync(mintB.publicKey, trader.publicKey, false, TOKEN_2022_PROGRAM_ID);
        const ix = await (program.methods as any)
          .claimReward(trader.publicKey, CLAIM_AMOUNT, CLAIM_AMOUNT)
          .accountsStrict({
            pool: addresses.pool,
            claimer: trader.publicKey,
            toMintAAccount: traderA,
            toMintBAccount: traderB,
            mintA: mintA.publicKey,
            mintB: mintB.publicKey,
            rewardVault: addresses.rewardVault,
            rewardAccountA: addresses.rewardAccountA,
            rewardAccountB: addresses.rewardAccountB,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .instruction();
        const tx = new Transaction().add(memoIx(`loadtest:claim:${i}:${trader.publicKey.toBase58()}`), ix);
        await sendWithLogs({
          provider,
          tx,
          extraSigners: [trader],
          label: `claimReward iter ${i}`,
          skipPreflight: SKIP_PREFLIGHT,
          maxRetries: MAX_RETRIES,
          verbose: VERBOSE,
        });
        stats.claims++;
      }

      const traderA = getAssociatedTokenAddressSync(mintA.publicKey, trader.publicKey, false, TOKEN_2022_PROGRAM_ID);
      const traderB = getAssociatedTokenAddressSync(mintB.publicKey, trader.publicKey, false, TOKEN_2022_PROGRAM_ID);

      const trackedA = traderBalancesA.get(traderKey) ?? new anchor.BN(0);
      const trackedB = traderBalancesB.get(traderKey) ?? new anchor.BN(0);
      const needsA = swapA && trackedA.lt(SWAP_AMOUNT);
      const needsB = !swapA && trackedB.lt(SWAP_AMOUNT);
      if (needsA || needsB) {
        const topUpA = needsA ? INITIAL_A : new anchor.BN(0);
        const topUpB = needsB ? INITIAL_B : new anchor.BN(0);
        const topupOnce = async () => {
          const ix = await (program.methods as any)
            .claimReward(trader.publicKey, topUpA, topUpB)
            .accountsStrict({
              pool: addresses.pool,
              claimer: trader.publicKey,
              toMintAAccount: traderA,
              toMintBAccount: traderB,
              mintA: mintA.publicKey,
              mintB: mintB.publicKey,
              rewardVault: addresses.rewardVault,
              rewardAccountA: addresses.rewardAccountA,
              rewardAccountB: addresses.rewardAccountB,
              tokenProgram: TOKEN_2022_PROGRAM_ID,
              associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
              systemProgram: SystemProgram.programId,
            })
            .instruction();
          const tx = new Transaction().add(memoIx(`loadtest:topup:${i}:${traderKey}`), ix);
          await sendWithLogs({
            provider,
            tx,
            extraSigners: [trader],
            label: `topup iter ${i}`,
            skipPreflight: SKIP_PREFLIGHT,
            maxRetries: MAX_RETRIES,
            verbose: VERBOSE,
            });
        };
        try {
          await topupOnce();
        } catch (err: any) {
          if (
            err instanceof TxFailedError &&
            logsContain(err.logs, ["InsufficientReward", "Not enough tokens in the reward account"])
          ) {
            const depositIx = await (program.methods as any)
              .depositRewards(EMISSION_TEMP)
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
                systemProgram: SystemProgram.programId,
                mintAuthority: addresses.mintAuthority,
              })
              .instruction();
            const depositTx = new Transaction().add(memoIx(`loadtest:rewards:refill:${i}`), depositIx);
            await sendWithLogs({
              provider,
              tx: depositTx,
              label: `depositRewards refill iter ${i}`,
              skipPreflight: SKIP_PREFLIGHT,
              maxRetries: MAX_RETRIES,
              verbose: VERBOSE,
            });
            await topupOnce();
          } else {
            throw err;
          }
        }
        traderBalancesA.set(traderKey, trackedA.add(topUpA));
        traderBalancesB.set(traderKey, trackedB.add(topUpB));
      }

      let expectedOutput: number | null = null;
      if (SAMPLE_EVERY > 0 && i % SAMPLE_EVERY === 0) {
        const poolBalA = await provider.connection.getTokenAccountBalance(addresses.poolAccountA);
        const poolBalB = await provider.connection.getTokenAccountBalance(addresses.poolAccountB);
        const reserveA = new anchor.BN(poolBalA.value.amount);
        const reserveB = new anchor.BN(poolBalB.value.amount);
        const weights = temperatureWeights(currentTemperature);
        const reserveIn = swapA ? reserveA : reserveB;
        const reserveOut = swapA ? reserveB : reserveA;
        const weightIn = swapA ? weights.weightEnd : weights.weightGaia;
        const weightOut = swapA ? weights.weightGaia : weights.weightEnd;
        expectedOutput = expectedWeightedSwapOutput({
          inputAmount: SWAP_AMOUNT,
          reserveIn,
          reserveOut,
          weightIn,
          weightOut,
          feeBps,
        });
      }

      const t0 = Date.now();
      const swapIx = await (program.methods as any)
        .swap(swapA, SWAP_AMOUNT, new anchor.BN(1))
        .accountsStrict({
          amm: addresses.amm,
          poolAuthority: addresses.poolAuthority,
          trader: trader.publicKey,
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
        .instruction();
      const swapTx = new Transaction().add(
        memoIx(`loadtest:swap:${i}:${trader.publicKey.toBase58()}:${swapA ? "A" : "B"}`),
        swapIx
      );
      const swapOnce = async () =>
        await sendWithLogs({
          provider,
          tx: swapTx,
          extraSigners: [trader],
          label: `swap iter ${i}`,
          skipPreflight: true,
          maxRetries: MAX_RETRIES,
          verbose: VERBOSE,
        });
      let sig: string;
      try {
        sig = await swapOnce();
      } catch (err: any) {
        if (
          err instanceof TxFailedError &&
          logsContain(err.logs, ["InsufficientLiquidity", "Pool reserves are empty", "OutputTooSmall", "Output is below"])
        ) {
          const liqIx = await (program.methods as any)
            .depositLiquidity(EMISSION_TEMP)
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
              systemProgram: SystemProgram.programId,
              mintAuthority: addresses.mintAuthority,
            })
            .instruction();
          const liqTx = new Transaction().add(memoIx(`loadtest:liq:refill:${i}`), liqIx);
          await sendWithLogs({
            provider,
            tx: liqTx,
            label: `depositLiquidity refill iter ${i}`,
            skipPreflight: SKIP_PREFLIGHT,
            maxRetries: MAX_RETRIES,
            verbose: VERBOSE,
          });
          sig = await swapOnce();
        } else {
          throw err;
        }
      }
      const dt = Date.now() - t0;
      stats.latenciesMs.push(dt);
      stats.swaps++;

      // Update local balance model (best-effort; output depends on pool state so we only account for the input side).
      if (swapA) traderBalancesA.set(traderKey, (traderBalancesA.get(traderKey) ?? new anchor.BN(0)).sub(SWAP_AMOUNT));
      else traderBalancesB.set(traderKey, (traderBalancesB.get(traderKey) ?? new anchor.BN(0)).sub(SWAP_AMOUNT));

      if (SAMPLE_EVERY > 0 && i % SAMPLE_EVERY === 0) {
        const tx = await provider.connection.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
        const logs = tx?.meta?.logMessages ?? [];
        const parsed = parseSwapLog(logs);
        if (parsed) {
          const input = Number(parsed.input.toString());
          const output = Number(parsed.output.toString());
          if (input > 0) {
            stats.temperatureSamples.push({
              temperature: currentTemperature,
              outputPerInput: output / input,
            });
          }
          if (expectedOutput != null) {
            const absErr = Math.abs(output - expectedOutput);
            stats.fairnessAbsError.push(absErr);
          }
        }
        if (typeof tx?.meta?.computeUnitsConsumed === "number") {
          stats.computeUnits.push(tx.meta.computeUnitsConsumed);
        }
      }
    } catch (err: any) {
      stats.failures++;
      const msg = err?.message || err?.toString?.() || String(err);
      if (process.env.VERBOSE === "1") {
        console.error(`iter ${i} failed:`, msg);
      }
    }

    if (THROTTLE_MS > 0) {
      await sleepMs(THROTTLE_MS);
    }
  }

  const elapsed = (Date.now() - startAll) / 1000;
  stats.latenciesMs.sort((a, b) => a - b);
  stats.computeUnits.sort((a, b) => a - b);
  stats.fairnessAbsError.sort((a, b) => a - b);

  const summary = {
    iterations: ITERATIONS,
    traders: traders.length,
    swaps: stats.swaps,
    claims: stats.claims,
    liquidityDeposits: stats.liquidityDeposits,
    rewardDeposits: stats.rewardDeposits,
    failures: stats.failures,
    elapsedSeconds: elapsed,
    swapsPerSecond: stats.swaps / Math.max(1e-9, elapsed),
    latencyMs: {
      p50: percentile(stats.latenciesMs, 50),
      p90: percentile(stats.latenciesMs, 90),
      p99: percentile(stats.latenciesMs, 99),
      mean: mean(stats.latenciesMs),
      max: stats.latenciesMs.length ? stats.latenciesMs[stats.latenciesMs.length - 1] : 0,
    },
    computeUnits: {
      p50: percentile(stats.computeUnits, 50),
      p90: percentile(stats.computeUnits, 90),
      p99: percentile(stats.computeUnits, 99),
      mean: mean(stats.computeUnits),
    },
    fairness: {
      samples: stats.fairnessAbsError.length,
      meanAbsError: mean(stats.fairnessAbsError),
      p99AbsError: percentile(stats.fairnessAbsError, 99),
      maxAbsError: stats.fairnessAbsError.length ? stats.fairnessAbsError[stats.fairnessAbsError.length - 1] : 0,
    },
    temperature: {
      samples: stats.temperatureSamples.length,
      avgOutputPerInput: mean(stats.temperatureSamples.map((s) => s.outputPerInput)),
    },
    indexAccountsFile: path.relative(process.cwd(), INDEX_PATH),
  };

  process.stdout.write(JSON.stringify(summary, null, 2) + "\n");
}

main().catch((err) => {
  console.error(err?.message || err);
  process.exit(1);
});
