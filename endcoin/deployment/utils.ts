import * as anchor from "@coral-xyz/anchor";
import {
  createMint,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, Connection, Signer } from "@solana/web3.js";
import { BN } from "bn.js";
import { readFileSync } from "fs";
export async function sleep(seconds: number) {
  return new Promise((resolve) => setTimeout(resolve, seconds * 1000));
}

export const generateSeededKeypair = (seed: string) => {
  return Keypair.fromSeed(
    anchor.utils.bytes.utf8.encode(anchor.utils.sha256.hash(seed)).slice(0, 32)
  );
};

export const expectRevert = async (promise: Promise<any>) => {
  try {
    await promise;
    throw new Error("Expected a revert");
  } catch {
    return;
  }
};

export const mintingTokens = async ({
  connection,
  creator,
  holder = creator,
  mintAKeypair,
  mintBKeypair,
  mintedAmount = 100,
  decimals = 6,
}: {
  connection: Connection;
  creator: Signer;
  holder?: Signer;
  mintAKeypair: Keypair;
  mintBKeypair: Keypair;
  mintedAmount?: number;
  decimals?: number;
}) => {
  // Mint tokens
  await createMint(
    connection,
    creator,
    creator.publicKey,
    creator.publicKey,
    decimals,
    mintAKeypair,
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
  await createMint(
    connection,
    creator,
    creator.publicKey,
    creator.publicKey,
    decimals,
    mintBKeypair,
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
  await getOrCreateAssociatedTokenAccount(
    connection,
    holder,
    mintAKeypair.publicKey,
    holder.publicKey,
    true,
    "confirmed",
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
  await getOrCreateAssociatedTokenAccount(
    connection,
    holder,
    mintBKeypair.publicKey,
    holder.publicKey,
    true,
    "confirmed",
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
  await mintTo(
    connection,
    creator,
    mintAKeypair.publicKey,
    getAssociatedTokenAddressSync(
      mintAKeypair.publicKey,
      holder.publicKey,
      true,
      TOKEN_2022_PROGRAM_ID
    ),
    creator.publicKey,
    mintedAmount * 10 ** decimals,
    [],
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
  await mintTo(
    connection,
    creator,
    mintBKeypair.publicKey,
    getAssociatedTokenAddressSync(
      mintBKeypair.publicKey,
      holder.publicKey,
      true,
      TOKEN_2022_PROGRAM_ID
    ),
    creator.publicKey,
    mintedAmount * 10 ** decimals,
    [],
    undefined,
    TOKEN_2022_PROGRAM_ID
  );
};

export interface TestValues {
  fee: number;
  admin: Keypair;
  mintAKeypair: Keypair;
  mintBKeypair: Keypair;
  defaultSupply: anchor.BN;
  ammKey: PublicKey;
  minimumLiquidity: anchor.BN;
  poolKey: PublicKey;
  poolAuthority: PublicKey;
  mintLiquidityKeypair: Keypair;
  mintLiquidity: PublicKey;
  depositAmountA: anchor.BN;
  depositAmountB: anchor.BN;
  liquidityAccount: PublicKey;
  poolAccountA: PublicKey;
  poolAccountB: PublicKey;
  holderAccountA: PublicKey;
  holderAccountB: PublicKey;
  endcoinMetadata: PublicKey,
  gaiacoinMetadata: PublicKey, 
}

type TestValuesDefaults = {
  [K in keyof TestValues]+?: TestValues[K];
};
export function createValues(defaults?: TestValuesDefaults): TestValues {

  let admin = anchor.AnchorProvider.env().wallet.payer;

  const ammKey = PublicKey.findProgramAddressSync(
    [Buffer.from("amm")],
    anchor.workspace.Endcoin.programId
  )[0];

  // Making sure tokens are in the right order
  // load keypairs from json files (support raw array or named property)
  const endcoinRaw = JSON.parse(
    readFileSync(
      "./deployment/keys/ENDxPmLfBBTVby7DBYUo4gEkFABQgvLP2LydFCzGGBee.json",
      "utf8"
    )
  );
  const endcoinSecret: number[] = Array.isArray(endcoinRaw)
    ? endcoinRaw
    : endcoinRaw.endcoin_mint;
  const mintAKeypair = Keypair.fromSecretKey(
    Uint8Array.from(endcoinSecret)
  );

  const gaiacoinRaw = JSON.parse(
    readFileSync(
      "./deployment/keys/GAiAxUPQrUaELAuri8tVC354bGuUGGykCN8tP4qfCeSp.json",
      "utf8"
    )
  );
  const gaiacoinSecret: number[] = Array.isArray(gaiacoinRaw)
    ? gaiacoinRaw
    : gaiacoinRaw.gaiacoin_mint;
  let mintBKeypair = Keypair.fromSecretKey(Uint8Array.from(gaiacoinSecret));

  const poolAuthority = PublicKey.findProgramAddressSync(
    [
      ammKey.toBuffer(),
      mintAKeypair.publicKey.toBuffer(),
      mintBKeypair.publicKey.toBuffer(),
      Buffer.from("pool-authority"),
    ],
    anchor.workspace.Endcoin.programId
  )[0];
  

  const mintLiquidityRaw = JSON.parse(
    readFileSync(
      "./deployment/keys/PLSxiYHus8rhc2NhXs2qvvhAcpsa4Q3TzTCi3o8xAEU.json",
      "utf8"
    )
  );
  const liquiditySecret: number[] = Array.isArray(mintLiquidityRaw)
    ? mintLiquidityRaw
    : mintLiquidityRaw.liquidity_mint;
  let mintLiquidityKeypair = Keypair.fromSecretKey(Uint8Array.from(liquiditySecret));
  const mintLiquidity = mintLiquidityKeypair.publicKey;
  
  const poolKey = PublicKey.findProgramAddressSync(
    [
      ammKey.toBuffer(),
      mintAKeypair.publicKey.toBuffer(),
      mintBKeypair.publicKey.toBuffer(),
    ],
    anchor.workspace.Endcoin.programId
  )[0];
  
  const endcoinMetadata = PublicKey.findProgramAddressSync(
    [
      Buffer.from("endcoin_metadata")
    ],
    anchor.workspace.Endcoin.programId
  )[0];
  
  const gaiacoinMetadata = PublicKey.findProgramAddressSync(
    [
      Buffer.from("gaiacoin_metadata")
    ],
    anchor.workspace.Endcoin.programId
  )[0];
  
  return {
    fee: 500,
    admin,
    ammKey,
    mintAKeypair,
    mintBKeypair,
    mintLiquidityKeypair,
    mintLiquidity,
    poolKey,
    poolAuthority,
    endcoinMetadata,
    gaiacoinMetadata, 
    poolAccountA: getAssociatedTokenAddressSync(
      mintAKeypair.publicKey,
      poolAuthority,
      true,
      TOKEN_2022_PROGRAM_ID
    ),
    poolAccountB: getAssociatedTokenAddressSync(
      mintBKeypair.publicKey,
      poolAuthority,
      true,
      TOKEN_2022_PROGRAM_ID
    ),
    liquidityAccount: getAssociatedTokenAddressSync(
      mintLiquidity,
      admin.publicKey,
      true
    ),
    holderAccountA: getAssociatedTokenAddressSync(
      mintAKeypair.publicKey,
      admin.publicKey,
      true,
      TOKEN_2022_PROGRAM_ID
    ),
    holderAccountB: getAssociatedTokenAddressSync(
      mintBKeypair.publicKey,
      admin.publicKey,
      true,
      TOKEN_2022_PROGRAM_ID
    ),
    depositAmountA: new BN(4 * 10 ** 6),
    depositAmountB: new BN(1 * 10 ** 6),
    minimumLiquidity: new BN(100),
    defaultSupply: new BN(100 * 10 ** 6),
  };
}
