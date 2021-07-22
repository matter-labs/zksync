/* eslint-disable sort-keys */
const { assert } = require('@polkadot/util');
const INIT_ERRROR = 'The WASM interface has not been initialized. Ensure that you wait for the initialization Promise with zksync_crypto_init() from zksync crypto before attempting to use WASM-only interfaces.';

module.exports = function (stubbed) {
    const wrapReady = (fn) =>
        (...params) => {
            assert(stubbed.isReady(), `fn: ${fn} ${INIT_ERRROR}`);
            return fn(...params);
        };

    return {
        zksync_crypto_init: wrapReady(stubbed.zksync_crypto_init),
        privateKeyFromSeed: wrapReady(stubbed.privateKeyFromSeed),
        private_key_to_pubkey_hash: wrapReady(stubbed.private_key_to_pubkey_hash),
        private_key_to_pubkey: wrapReady(stubbed.private_key_to_pubkey),
        sign_musig: wrapReady(stubbed.sign_musig),
        __wbindgen_malloc: wrapReady(stubbed.__wbindgen_malloc),
        __wbindgen_free: wrapReady(stubbed.__wbindgen_free),
        __wbindgen_realloc: wrapReady(stubbed.__wbindgen_realloc),

        isReady: stubbed.isReady,
        waitReady: stubbed.waitReady
    }
}