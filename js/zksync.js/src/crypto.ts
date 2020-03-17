import BN = require("bn.js");
import { Signature } from "./types";

const zksync_crypto = (async () => {
    if (typeof window !== "undefined" && window.window === window) {
        return await import("zksync-crypto");
    } else {
        return await import("zksync-crypto-node");
    }
})();

export async function signTransactionBytes(privKey: BN, bytes: Buffer): Promise<Signature> {
    const { sign_musig_sha256 } = await zksync_crypto;
    const signaturePacked = sign_musig_sha256(privKey.toArrayLike(Buffer), bytes);
    const pubKey = Buffer.from(signaturePacked.slice(0,32)).toString("hex");
    const signature = Buffer.from(signaturePacked.slice(32, 32 + 64)).toString("hex");
    return {
        pubKey,
        signature,
    };
}

export async function privateKeyFromSeed(seed: Buffer): Promise<BN> {
    const { private_key_from_seed } = await zksync_crypto;
    return new BN(private_key_from_seed(seed)); 
}

export async function privateKeyToPubKeyHash(privateKey: BN): Promise<string> {
    const { private_key_to_pubkey_hash } = await zksync_crypto;
    return `sync:${Buffer.from(private_key_to_pubkey_hash(privateKey.toArrayLike(Buffer))).toString("hex")}`
}
