import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import { terser } from 'rollup-plugin-terser';

function resolveWithZksyncCryptoReplace(options) {
  return resolve({
    ...options,
    resolveId: async (source, importer) => {
      const defaultResolveResult = await options.resolveId(source, importer);
      if (source === 'zksync-crypto') {
        defaultResolveResult.id = defaultResolveResult.id.replace('zksync-crypto-bundler', 'zksync-crypto-web');
      }
      return defaultResolveResult;
    },
  });
}

export default [
  {
    input: 'build/index.js',
    output: {
      file: 'dist/main.js',
      format: 'iife',
      name: 'zksync',
      globals: {
        ethers: 'ethers',
      },
    },
    external: ['ethers'],
    plugins: [
      resolveWithZksyncCryptoReplace({
        browser: true,
      }),
      commonjs(),
      json(),
      terser(),
    ],
  },
];

