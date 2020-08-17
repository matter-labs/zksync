
module.exports = {
  entry: './build/index.js',
  mode: 'production',
  experiments: {
      asyncWebAssembly: true
  },
  resolve: {
      alias: { util: false },
  },
};
