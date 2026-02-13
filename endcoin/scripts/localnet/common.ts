import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  getMint,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import os from "os";
import path from "path";

export const DEFAULT_DECIMALS = 6;
export const DEFAULT_ORACLE_FEED = new PublicKey(
  process.env.SST_ORACLE_FEED ?? "GhCs7zhha7kTyt8EiaWaBT5DREt23GnoPnqa7AU4yv1y"
);

type StoredKeypair = number[];

type LocalnetStateV1 = {
  version: 1;
  mintA: StoredKeypair;
  mintB: StoredKeypair;
  mintLiquidity: StoredKeypair;
};

export type LocalnetAddresses = {
  mintAuthority: PublicKey;
  amm: PublicKey;
  pool: PublicKey;
  poolAuthority: PublicKey;
  rewardVault: PublicKey;
  sst: PublicKey;
  oracleFeed: PublicKey;
  poolAccountA: PublicKey;
  poolAccountB: PublicKey;
  rewardAccountA: PublicKey;
  rewardAccountB: PublicKey;
};

export type LocalnetContext = {
  provider: anchor.AnchorProvider;
  program: Program;
  payer: Keypair;
  mintA: Keypair;
  mintB: Keypair;
  mintLiquidity: Keypair;
  addresses: LocalnetAddresses;
};

const LOCALNET_DIR = path.resolve(__dirname, "../../.localnet");
const STATE_PATH = path.join(LOCALNET_DIR, "state.json");
const ADDRESSES_PATH = path.join(LOCALNET_DIR, "addresses.json");
const IDL_PATH = path.resolve(__dirname, "../../target/idl/endcoin.json");

function readJsonFile<T>(filePath: string): T {
  return JSON.parse(readFileSync(filePath, "utf8")) as T;
}

function writeJsonFile(filePath: string, data: unknown) {
  writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n", "utf8");
}

function ensureLocalnetDir() {
  if (!existsSync(LOCALNET_DIR)) {
    mkdirSync(LOCALNET_DIR, { recursive: true });
  }
}

function keypairToStored(keypair: Keypair): StoredKeypair {
  return Array.from(keypair.secretKey);
}

function keypairFromStored(stored: StoredKeypair): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(stored));
}

function loadIdlFromDisk(): anchor.Idl {
  const raw = JSON.parse(readFileSync(IDL_PATH, "utf8"));
  return raw as anchor.Idl;
}

function expandHome(filePath: string): string {
  if (!filePath.startsWith("~")) return filePath;
  return path.join(os.homedir(), filePath.slice(1));
}

function resolveWalletPath(): string {
  const candidates = [
    process.env.ANCHOR_WALLET,
    "~/.config/solana/wba-wallet.json",
    "~/.config/solana/id.json",
  ]
    .filter((entry): entry is string => !!entry)
    .map(expandHome);

  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }

  throw new Error(
    `Unable to locate a wallet keypair. Tried: ${candidates.join(", ")}`
  );
}

export function getProviderAndProgram(): { provider: anchor.AnchorProvider; program: Program } {
  let provider: anchor.AnchorProvider;
  try {
    const envProvider = anchor.AnchorProvider.env();
    // Use at-least-confirmed commitment so helpers like `SendTransactionError.getLogs()`
    // can fetch confirmed transactions for debugging.
    const connection = new anchor.web3.Connection(envProvider.connection.rpcEndpoint, {
      commitment: "confirmed",
    });
    provider = new anchor.AnchorProvider(connection, envProvider.wallet, {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
    });
  } catch {
    const rpcEndpoint = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
    const walletPath = resolveWalletPath();
    const secretKey = Uint8Array.from(JSON.parse(readFileSync(walletPath, "utf8")));
    const wallet = new anchor.Wallet(anchor.web3.Keypair.fromSecretKey(secretKey));
    const connection = new anchor.web3.Connection(rpcEndpoint, {
      commitment: "confirmed",
    });
    provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
    });
  }

  anchor.setProvider(provider);
  // Standard practice for scripts: load the built IDL from disk so it matches what was just deployed.
  const idl = loadIdlFromDisk();
  const programIdStr = (idl as any).address;
  if (!programIdStr) {
    throw new Error(`Unable to find programId in IDL at ${IDL_PATH}`);
  }
  const program = new anchor.Program(idl, provider);
  return { provider, program };
}

export async function maybeAirdropLocalnet(
  provider: anchor.AnchorProvider,
  minLamports: number = 2 * LAMPORTS_PER_SOL
) {
  const rpcUrl = provider.connection.rpcEndpoint;
  const isLocal = rpcUrl.includes("127.0.0.1") || rpcUrl.includes("localhost");
  if (!isLocal) return;

  const current = await provider.connection.getBalance(provider.wallet.publicKey);
  if (current >= minLamports) return;

  const sig = await provider.connection.requestAirdrop(provider.wallet.publicKey, minLamports - current);
  const latest = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");
}

export function deriveAddresses(
  programId: PublicKey,
  mintA: PublicKey,
  mintB: PublicKey
): Omit<LocalnetAddresses, "poolAccountA" | "poolAccountB" | "rewardAccountA" | "rewardAccountB"> {
  const [mintAuthority] = PublicKey.findProgramAddressSync([Buffer.from("authority")], programId);
  const [amm] = PublicKey.findProgramAddressSync([Buffer.from("amm")], programId);

  const [pool] = PublicKey.findProgramAddressSync(
    [amm.toBuffer(), mintA.toBuffer(), mintB.toBuffer()],
    programId
  );
  const [poolAuthority] = PublicKey.findProgramAddressSync(
    [amm.toBuffer(), mintA.toBuffer(), mintB.toBuffer(), Buffer.from("pool-authority")],
    programId
  );
  const [rewardVault] = PublicKey.findProgramAddressSync(
    [pool.toBuffer(), mintA.toBuffer(), mintB.toBuffer(), Buffer.from("reward-vault")],
    programId
  );
  const [sst] = PublicKey.findProgramAddressSync(
    // Canonical SST for the AMM (prevents users from spoofing temperature by choosing a different payer).
    [Buffer.from("sea-surface-temperature"), amm.toBuffer()],
    programId
  );

  return { mintAuthority, amm, pool, poolAuthority, rewardVault, sst, oracleFeed: DEFAULT_ORACLE_FEED };
}

export function deriveTokenAddresses(
  mintA: PublicKey,
  mintB: PublicKey,
  poolAuthority: PublicKey,
  rewardVault: PublicKey
): Pick<LocalnetAddresses, "poolAccountA" | "poolAccountB" | "rewardAccountA" | "rewardAccountB"> {
  const poolAccountA = getAssociatedTokenAddressSync(mintA, poolAuthority, true, TOKEN_2022_PROGRAM_ID);
  const poolAccountB = getAssociatedTokenAddressSync(mintB, poolAuthority, true, TOKEN_2022_PROGRAM_ID);
  const rewardAccountA = getAssociatedTokenAddressSync(mintA, rewardVault, true, TOKEN_2022_PROGRAM_ID);
  const rewardAccountB = getAssociatedTokenAddressSync(mintB, rewardVault, true, TOKEN_2022_PROGRAM_ID);
  return { poolAccountA, poolAccountB, rewardAccountA, rewardAccountB };
}

async function accountExists(connection: anchor.web3.Connection, pubkey: PublicKey) {
  return !!(await connection.getAccountInfo(pubkey));
}

export async function ensureToken2022Ata(params: {
  connection: anchor.web3.Connection;
  payer: Keypair;
  mint: PublicKey;
  owner: PublicKey;
}): Promise<PublicKey> {
  const { connection, payer, mint, owner } = params;
  const ata = getAssociatedTokenAddressSync(mint, owner, true, TOKEN_2022_PROGRAM_ID);
  const existing = await connection.getAccountInfo(ata);
  if (existing) return ata;

  const instruction = createAssociatedTokenAccountInstruction(
    payer.publicKey,
    ata,
    owner,
    mint,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  const tx = new Transaction().add(instruction);
  await sendAndConfirmTransaction(connection, tx, [payer], { commitment: "confirmed" });
  return ata;
}

async function ensureToken2022Mint(params: {
  connection: anchor.web3.Connection;
  payer: Keypair;
  mintKeypair: Keypair;
  mintAuthority: PublicKey;
  decimals: number;
  label: string;
}) {
  const { connection, payer, mintKeypair, mintAuthority, decimals, label } = params;
  const info = await connection.getAccountInfo(mintKeypair.publicKey);
  if (!info) {
    await createMint(
      connection,
      payer,
      mintAuthority,
      null,
      decimals,
      mintKeypair,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    return;
  }
  if (!info.owner.equals(TOKEN_2022_PROGRAM_ID)) {
    throw new Error(
      `${label} mint ${mintKeypair.publicKey.toBase58()} exists but is owned by ${info.owner.toBase58()}, expected Token-2022 ${TOKEN_2022_PROGRAM_ID.toBase58()}`
    );
  }

  const onchain = await getMint(connection, mintKeypair.publicKey, "confirmed", TOKEN_2022_PROGRAM_ID);
  const onchainAuthority = onchain.mintAuthority;
  if (!onchainAuthority || !onchainAuthority.equals(mintAuthority)) {
    throw new Error(
      `${label} mint authority mismatch for ${mintKeypair.publicKey.toBase58()}: on-chain ${
        onchainAuthority ? onchainAuthority.toBase58() : "null"
      } vs expected ${mintAuthority.toBase58()}`
    );
  }
  if (onchain.decimals !== decimals) {
    throw new Error(
      `${label} decimals mismatch for ${mintKeypair.publicKey.toBase58()}: on-chain ${onchain.decimals} vs expected ${decimals}`
    );
  }
}

export function loadOrCreateLocalnetState(): LocalnetStateV1 {
  ensureLocalnetDir();
  if (existsSync(STATE_PATH)) {
    const state = readJsonFile<LocalnetStateV1>(STATE_PATH);
    if (state.version !== 1) {
      throw new Error(`Unsupported localnet state version: ${(state as any).version}`);
    }
    return state;
  }

  const mintA = Keypair.generate();
  const mintB = Keypair.generate();
  const mintLiquidity = Keypair.generate();

  const state: LocalnetStateV1 = {
    version: 1,
    mintA: keypairToStored(mintA),
    mintB: keypairToStored(mintB),
    mintLiquidity: keypairToStored(mintLiquidity),
  };
  writeJsonFile(STATE_PATH, state);
  return state;
}

export async function ensureLocalnetInitialized(params?: {
  feeBps?: number;
  decimals?: number;
}): Promise<LocalnetContext> {
  const feeBps = params?.feeBps ?? 500;
  const decimals = params?.decimals ?? DEFAULT_DECIMALS;

  const { provider, program } = getProviderAndProgram();
  await maybeAirdropLocalnet(provider);

  const payer = provider.wallet.payer;
  const state = loadOrCreateLocalnetState();

  const mintA = keypairFromStored(state.mintA);
  const mintB = keypairFromStored(state.mintB);
  const mintLiquidity = keypairFromStored(state.mintLiquidity);

  const core = deriveAddresses(program.programId, mintA.publicKey, mintB.publicKey);
  const tokenAccounts = deriveTokenAddresses(mintA.publicKey, mintB.publicKey, core.poolAuthority, core.rewardVault);
  const addresses: LocalnetAddresses = { ...core, ...tokenAccounts };

  await ensureToken2022Mint({
    connection: provider.connection,
    payer,
    mintKeypair: mintA,
    mintAuthority: addresses.mintAuthority,
    decimals,
    label: "mintA",
  });

  await ensureToken2022Mint({
    connection: provider.connection,
    payer,
    mintKeypair: mintB,
    mintAuthority: addresses.mintAuthority,
    decimals,
    label: "mintB",
  });

  await ensureToken2022Mint({
    connection: provider.connection,
    payer,
    mintKeypair: mintLiquidity,
    mintAuthority: addresses.poolAuthority,
    decimals,
    label: "mintLiquidity",
  });

  if (!(await accountExists(provider.connection, addresses.amm))) {
    await program.methods
      .createAmm(feeBps)
      .accountsStrict({
        amm: addresses.amm,
        admin: payer.publicKey,
        authority: payer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  }

  const ammAccount: any = await (program as any).account.amm.fetch(addresses.amm);
  if (!ammAccount.admin.equals(payer.publicKey)) {
    throw new Error(
      `AMM admin mismatch: on-chain ${ammAccount.admin.toBase58()} vs expected ${payer.publicKey.toBase58()}`
    );
  }

  if (!(await accountExists(provider.connection, addresses.pool))) {
    await program.methods
      .createPool()
      .accountsStrict({
        amm: addresses.amm,
        pool: addresses.pool,
        poolAuthority: addresses.poolAuthority,
        mintLiquidity: mintLiquidity.publicKey,
        mintA: mintA.publicKey,
        mintB: mintB.publicKey,
        payer: payer.publicKey,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .rpc();
  }

  if (!(await accountExists(provider.connection, addresses.sst))) {
    await program.methods
      .createSst(addresses.oracleFeed)
      // Cast to `any` so this script remains runnable even if `target/types` hasn't been regenerated yet.
      .accountsStrict({
        amm: addresses.amm,
        sst: addresses.sst,
        admin: payer.publicKey,
        payer: payer.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();
  }

  if (!(await accountExists(provider.connection, addresses.rewardVault))) {
    await program.methods
      .createRewardVault(payer.publicKey)
      .accountsStrict({
        rewardVault: addresses.rewardVault,
        pool: addresses.pool,
        amm: addresses.amm,
        mintA: mintA.publicKey,
        mintB: mintB.publicKey,
        payer: payer.publicKey,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .rpc();
  }

  await ensureToken2022Ata({
    connection: provider.connection,
    payer,
    mint: mintA.publicKey,
    owner: addresses.poolAuthority,
  });
  await ensureToken2022Ata({
    connection: provider.connection,
    payer,
    mint: mintB.publicKey,
    owner: addresses.poolAuthority,
  });
  await ensureToken2022Ata({
    connection: provider.connection,
    payer,
    mint: mintA.publicKey,
    owner: addresses.rewardVault,
  });
  await ensureToken2022Ata({
    connection: provider.connection,
    payer,
    mint: mintB.publicKey,
    owner: addresses.rewardVault,
  });

  writeJsonFile(ADDRESSES_PATH, {
    programId: program.programId.toBase58(),
    payer: payer.publicKey.toBase58(),
    mintA: mintA.publicKey.toBase58(),
    mintB: mintB.publicKey.toBase58(),
    mintLiquidity: mintLiquidity.publicKey.toBase58(),
    ...Object.fromEntries(Object.entries(addresses).map(([k, v]) => [k, v.toBase58()])),
  });

  return { provider, program, payer, mintA, mintB, mintLiquidity, addresses };
}

export function readLocalnetAddressesFile(): Record<string, string> | null {
  if (!existsSync(ADDRESSES_PATH)) return null;
  return readJsonFile<Record<string, string>>(ADDRESSES_PATH);
}
