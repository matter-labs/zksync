// Prefer the native Node.js wasm-pack build.
// Keep asm.js fallback as an opt-in escape hatch only.

try {
    module.exports = require('./dist/zksync-crypto-node.js');
} catch (e) {
    if (process.env.ZKSYNC_USE_ASM_FALLBACK === '1') {
        module.exports = require('./dist/zksync-crypto-bundler_asm.js');
    } else {
        throw e;
    }
}
