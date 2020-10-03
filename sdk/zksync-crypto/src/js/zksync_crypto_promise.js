
const asm = require('./zksync_crypto_asm_stub');
const bytes = require('./zksync_crypto_wasm');
const imports = require('./zksync_crypto');

module.exports = async function createExportPromise () {
    try {
        const { instance } = await WebAssembly.instantiate(bytes, { __wbindgen_placeholder__: imports });

        return instance.exports;
    } catch (error) {
        // if we have a valid supplied asm.js, return that
        console.log("asm", asm.zksync_crypto_init);
        if (asm && asm.zksync_crypto_init) {
            return asm;
        }

        console.error(`ERROR: Unable to instantiate WebAssembly & initialize zksync-crypto`);
        console.error(error);

        return null;
    }
};
