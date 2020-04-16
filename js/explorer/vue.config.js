module.exports = {
    publicPath: '/',
    chainWebpack: config => {
        config.optimization.minimize(process.env.NODE_ENV === 'production');
        config.resolve.symlinks(false);
    },
};
