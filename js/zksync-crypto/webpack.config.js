const HtmlWebpackPlugin = require('html-webpack-plugin');
const path = require('path');
const webpack = require('webpack');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

const config = target => ({
    entry: './indexx.js',
    output: {
        path: path.resolve(__dirname, 'dist'),
        filename: `index.${target}.js`,
        libraryTarget: 'umd',
    },
    plugins: [
        new HtmlWebpackPlugin(),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, ".")
        }),
        // Have this example work in Edge which doesn't ship `TextEncoder` or
        // `TextDecoder` at this time.
        new webpack.ProvidePlugin({
          TextDecoder: ['text-encoding', 'TextDecoder'],
          TextEncoder: ['text-encoding', 'TextEncoder']
        })
    ],
    mode: 'development',
    // module: {
    //     rules: [
    //         { test: /\.wasm$/, type: "webassembly/experimental" },
    //     ],
    // },
    // devServer: {
    //     mimeTypes: { 'text/html': ['wasm'] }
    // },
});

module.exports = ['web', 'node'].map(target => ({...config(target), target}));
