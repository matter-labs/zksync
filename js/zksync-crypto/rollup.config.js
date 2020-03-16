import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import { terser } from 'rollup-plugin-terser';
import wasm from '@rollup/plugin-wasm';
import typescript from '@rollup/plugin-typescript';
import dts from "rollup-plugin-dts";

// `npm run build` -> `production` is true
// `npm run dev` -> `production` is false
const production = !process.env.ROLLUP_WATCH;

export default {
    input: [
        "pkg/zksync_crypto.js",
    ],
	output: {
		file: 'public/bundle.js',
		format: 'cjs'
	},
	plugins: [
        resolve(), // tells Rollup how to find date-fns in node_modules
        dts(),
        commonjs(), // converts date-fns to ES modules
        wasm(),
		production && terser() // minify, but only in production
	]
};
