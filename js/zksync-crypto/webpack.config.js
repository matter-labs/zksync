const HtmlWebpackPlugin = require('html-webpack-plugin');
const path = require('path');
const webpack = require('webpack');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

const config = target => ({
    entry: './indexx.js',
    // entry: `./index_${target}.js`,
    output: {
        path: path.resolve(__dirname, 'dist'),
        filename: `index.${target}.js`,
        libraryTarget: 'umd',
    },
    plugins: [
        new HtmlWebpackPlugin(),
        new WasmPackPlugin({
            crateDirectory: path.resolve(__dirname, "."),
            extraArgs
                : target == 'web'  ? ''
                : target == 'node' ? '--target=nodejs'
                : null,
        }),
        // Have this example work in Edge which doesn't ship `TextEncoder` or
        // `TextDecoder` at this time.
        new webpack.ProvidePlugin({
          TextDecoder: ['text-encoding', 'TextDecoder'],
          TextEncoder: ['text-encoding', 'TextEncoder']
        })
    ],
    mode: 'development',
});

module.exports = ['web', 'node'].map(target => ({...config(target), target}));
