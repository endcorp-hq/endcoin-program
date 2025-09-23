import * as anchor from "@coral-xyz/anchor";

const jsonfile = require('jsonfile');
const file = './deployment/keys.json'

function generateKeypairs() {
    // gaiacoin mint public key
    const END_KEYPAIR = new anchor.web3.Keypair();
    const END_MINT = END_KEYPAIR.publicKey.toBase58();
    const GAIA_KEYPAIR = new anchor.web3.Keypair();
    const GAIA_MINT = GAIA_KEYPAIR.publicKey.toBase58();
    const LP_KEYPAIR = new anchor.web3.Keypair();
    const LP_MINT = LP_KEYPAIR.publicKey.toBase58();
    const AMM = new anchor.web3.Keypair();
    const AMM_PUBKEY = AMM.publicKey.toBase58();
    const POOL = new anchor.web3.Keypair();
    const POOL_PUBKEY = POOL.publicKey.toBase58();

    let keypairs = {
        endcoin_mint: END_MINT,
        gaiacoin_mint: GAIA_MINT,
        lp_mint: LP_MINT,
        amm: AMM_PUBKEY,
        pool: POOL_PUBKEY
    
    };

    jsonfile.writeFile(file, keypairs, function (err) {
        if (err) console.error(err)
    });
};

generateKeypairs();