import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import { terser } from 'rollup-plugin-terser';

function resolveWithZksyncCryptoReplace(options) {
    const plugin = resolve(options);
    const defaultPluginResolveId = plugin.resolveId;
    plugin.resolveId = async (source, importer) => {
        const defaultResolveResult = await defaultPluginResolveId(source, importer);
        if (source === 'zksync-crypto') {
            defaultResolveResult.id = defaultResolveResult.id.replace('zksync-crypto-bundler', 'zksync-crypto-web');
        }
        return defaultResolveResult;
    };
    return plugin;
}

export default [
    {
        input: 'build/index.js',
        output: {
            file: 'build/main.js',
            format: 'iife',
            name: 'zksync',
            globals: {
                ethers: 'ethers'
            }
        },
        external: ['ethers'],
        plugins: [
            resolveWithZksyncCryptoReplace({
                browser: true
            }),
            commonjs(),
            json(),
            terser()
        ]
    }
];
