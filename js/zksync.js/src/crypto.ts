import { Signature } from "./types";

import { private_key_to_pubkey_hash, sign_musig_rescue } from "zksync-crypto";

export { privateKeyFromSeed } from "zksync-crypto";

export function signTransactionBytes(
    privKey: Uint8Array,
    bytes: Uint8Array
): Signature {
    const signaturePacked = sign_musig_rescue(privKey, bytes);
    const pubKey = Buffer.from(signaturePacked.slice(0, 32)).toString("hex");
    const signature = Buffer.from(signaturePacked.slice(32, 32 + 64)).toString(
        "hex"
    );
    return {
        pubKey,
        signature
    };
}

export function privateKeyToPubKeyHash(privateKey: Uint8Array): string {
    return `sync:${Buffer.from(private_key_to_pubkey_hash(privateKey)).toString(
        "hex"
    )}`;
}
