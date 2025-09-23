import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
// no SystemProgram import needed
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import { SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { PublicKey } from "@solana/web3.js";
import { Endcoin } from "../target/types/endcoin";
import { TestValues, createValues, mintingTokens } from "./utils";

async function run() {
  const provider = anchor.AnchorProvider.env();
  const connection = provider.connection;
  anchor.setProvider(provider);

  const program = anchor.workspace.Endcoin as Program<Endcoin>;

  let values: TestValues = createValues();

  console.log('payer balance',
    await connection.getBalance(provider.wallet.publicKey),
    'admin balance',
    await connection.getBalance(values.admin.publicKey)
  );

  console.log('adm key', values.admin.publicKey.toBase58())


  // Init Reward Vault (payer must be AMM admin per on-chain checks)
  const rewardVault = PublicKey.findProgramAddressSync(
    [
      values.poolKey.toBuffer(),
      values.mintAKeypair.publicKey.toBuffer(),
      values.mintBKeypair.publicKey.toBuffer(),
      Buffer.from("reward-vault"),
    ],
    program.programId
  )[0];

  await program.methods
    .createRewardVault()
    .accountsStrict({
      rewardVault,
      pool: values.poolKey,
      amm: values.ammKey,
      mintA: values.mintAKeypair.publicKey,
      mintB: values.mintBKeypair.publicKey,
      payer: provider.wallet.publicKey, 
      systemProgram: SystemProgram.programId,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    })
    .signers([provider.wallet.payer])
    .rpc({ skipPreflight: false });

  console.log("Reward Vault created:", rewardVault.toBase58());
}

run().catch((err) => {
  console.error("create-reward failed:", err.toString());
  process.exit(1);
});
