import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import { Endcoin } from "../target/types/endcoin";
import { TestValues, createValues, expectRevert, mintingTokens } from "./utils";
import { getMint, getAssociatedTokenAddressSync } from "@solana/spl-token";
// metaplex token metadata program ID
const TOKEN_METADATA_PROGRAM_ID = new anchor.web3.PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);
async function create_metadata() {

  

  const provider = anchor.AnchorProvider.env();
  const connection = provider.connection;
  anchor.setProvider(provider);

  const program = anchor.workspace.Endcoin as Program<Endcoin>;

let mintAKeypair = anchor.web3.Keypair.generate();

let mintBKeypair = anchor.web3.Keypair.generate();

let authority = anchor.web3.Keypair.generate();

let payer = anchor.web3.Keypair.generate();

let metadata = anchor.web3.Keypair.generate();

let endcoin = 0;
let gaiacoin = 1;

    console.log("Mint A: " + mintAKeypair.publicKey.toBase58())
    console.log("Mint B: " + mintBKeypair.publicKey.toBase58())
    console.log("Authority: " + authority.publicKey.toBase58())
    console.log("Payer: " + payer.publicKey.toBase58())

    await program.methods
      .createMetadata(endcoin).accounts({
        mint: mintAKeypair.publicKey,
        metadata: metadata.publicKey,
        mintAuthority: authority.publicKey,
        payer: payer.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
        sysvarInstruction: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }).signers([mintAKeypair, payer])
      .rpc({ skipPreflight: true });


    await program.methods
      .createMetadata(gaiacoin).accounts({
        mint: mintBKeypair.publicKey,
        metadata: metadata.publicKey,
        mintAuthority: authority.publicKey,
        payer: payer.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
        sysvarInstruction: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }).signers([mintBKeypair, payer])
      .rpc({ skipPreflight: true });


};

create_metadata();