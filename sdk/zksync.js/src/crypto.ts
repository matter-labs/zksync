import { Signature } from "./types";
import { private_key_to_pubkey_hash, sign_musig, waitReady, zksync_crypto_init } from "@teamnumio/zksync-crypto";
import { utils } from "ethers";

export { privateKeyFromSeed } from "@teamnumio/zksync-crypto";

export function signTransactionBytes(privKey: Uint8Array, bytes: Uint8Array): Signature {
    const signaturePacked = sign_musig(privKey, bytes);
    const pubKey = utils.hexlify(signaturePacked.slice(0, 32)).substr(2);
    const signature = utils.hexlify(signaturePacked.slice(32)).substr(2);
    return {
        pubKey,
        signature
    };
}

export function privateKeyToPubKeyHash(privateKey: Uint8Array): string {
    return `sync:${utils.hexlify(private_key_to_pubkey_hash(privateKey)).substr(2)}`;
}

let zksyncCryptoLoaded = false;

export async function loadZkSyncCrypto() {
    if (!zksyncCryptoLoaded) {
        await waitReady();
        await zksync_crypto_init();
        zksyncCryptoLoaded = true;
    }
}
