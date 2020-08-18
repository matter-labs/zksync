import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import copy from 'rollup-plugin-copy';
import { terser } from "rollup-plugin-terser";

export default [
	{
		input: 'build/index.js',
		output: {
            file: 'dist/main.js',
            format: 'iife',
            name: 'zksync',
        },
		plugins: [
			resolve({
                browser: true,
            }),
			commonjs(),
            json(),
            copy({
                targets: [{ src: 'node_modules/zksync-crypto/dist/zksync-crypto-web_bg.wasm', dest: 'dist/'}],
                verbose: true
            }),
            terser(),
		]
	},
];
