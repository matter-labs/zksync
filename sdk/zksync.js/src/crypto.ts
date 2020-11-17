import { Signature } from './types';

import { private_key_to_pubkey_hash, sign_musig } from 'zksync-crypto';
import * as zks from 'zksync-crypto';
import { utils } from 'ethers';

export async function privateKeyFromSeed(seed: Uint8Array): Promise<Uint8Array> {
    await loadZkSyncCrypto();

    return zks.privateKeyFromSeed(seed);
}

export async function signTransactionBytes(privKey: Uint8Array, bytes: Uint8Array): Promise<Signature> {
    await loadZkSyncCrypto();

    const signaturePacked = sign_musig(privKey, bytes);
    const pubKey = utils.hexlify(signaturePacked.slice(0, 32)).substr(2);
    const signature = utils.hexlify(signaturePacked.slice(32)).substr(2);
    return {
        pubKey,
        signature
    };
}

export async function privateKeyToPubKeyHash(privateKey: Uint8Array): Promise<string> {
    await loadZkSyncCrypto();

    return `sync:${utils.hexlify(private_key_to_pubkey_hash(privateKey)).substr(2)}`;
}

let zksyncCryptoLoaded = false;
export async function loadZkSyncCrypto(wasmFileUrl?: string) {
    if (zksyncCryptoLoaded) {
        return;
    }
    // Only runs in the browser
    if ((zks as any).loadZkSyncCrypto) {
        // It is ok if wasmFileUrl is not specified.
        // Actually, typically it should not be specified,
        // since the content of the `.wasm` file is read
        // from the `.js` file itself.
        await (zks as any).loadZkSyncCrypto(wasmFileUrl);
        zksyncCryptoLoaded = true;
    }
}
