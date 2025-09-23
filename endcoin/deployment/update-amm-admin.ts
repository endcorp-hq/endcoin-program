import { Endcoin } from "../target/types/endcoin";
import { TestValues, createValues } from "./utils";
import { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { SystemProgram } from "@solana/web3.js";

const provider = anchor.AnchorProvider.env();
const connection = provider.connection;
anchor.setProvider(provider);
const program = anchor.workspace.Endcoin as Program<Endcoin>;

let values: TestValues = createValues();

// Create AMM  
async function update_amm() {
  const tx = await program.methods
    .updateAdmin(values.admin.publicKey)
    .accountsStrict({
      amm: values.ammKey,
      admin: values.admin.publicKey,
    })
    .signers([values.admin])
    .rpc({ skipPreflight: false });

  const ammAccount = await program.account.amm.fetch(values.ammKey);
  console.log("AMM updated:", {
    admin: ammAccount.admin.toBase58(),
    fee: ammAccount.fee,
    signature: tx,
  });
}

update_amm().catch((err) => {
  console.error("update_amm failed:", err.toString());
  process.exit(1);
});