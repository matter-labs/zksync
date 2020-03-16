import BN = require("bn.js");
import { Signature } from "./types";
import zksync_crypto from "example-node-wasm";
// import * as zksync_crypto from '../node_modules/zksync-crypto/zksync_crypto.js';
// console.log({zksync_crypto});
// zksync_crypto.then(console.log);

// init();
// const zksync_crypto = import("zksync-crypto");
// import * as scrscr from 'example-node-wasm';
// console.log({scrscr});

export async function signTransactionBytes(privKey: BN, bytes: Buffer): Promise<Signature> {
    const { sign_musig_sha256 } = await zksync_crypto;
    const signaturePacked = sign_musig_sha256(privKey.toBuffer(), bytes);
    const pubKey = Buffer.from(signaturePacked.slice(0,32)).toString("hex");
    const signature = Buffer.from(signaturePacked.slice(32, 32 + 64)).toString("hex");
    return {
        pubKey,
        signature,
    };
}

export async function privateKeyFromSeed(seed: Buffer): Promise<BN> {
    console.log({zksync_crypto: await zksync_crypto});
    const { private_key_from_seed } = await zksync_crypto;
    return new BN(private_key_from_seed(seed)); 
}

export async function privateKeyToPubKeyHash(privateKey: BN): Promise<string> {
    const { private_key_to_pubkey_hash } = await zksync_crypto;
    return `sync:${Buffer.from(private_key_to_pubkey_hash(privateKey.toBuffer())).toString("hex")}`
}
