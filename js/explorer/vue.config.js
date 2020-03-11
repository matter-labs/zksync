module.exports = {
    publicPath: process.env.NODE_ENV === 'production'
        ? '/explorer/'
        : '/',
    chainWebpack: config => {
        config.optimization.minimize(process.env.NODE_ENV === 'production');
        config.resolve.symlinks(false);
    },
};
