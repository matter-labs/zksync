module.exports = {
    root: true,
    env: {
        browser: true,
        node: true,
        es6: true,
        mocha: true
    },
    extends: ['alloy'],
    rules: {
        'no-console': 'off',
        'no-debugger': process.env.NODE_ENV === 'production' ? 'error' : 'off',
        // 'no-unused-vars': 'warn',
        semi: 'warn',
        'no-extra-semi': 'off',
        'no-empty': 'warn',
        'spaced-comment': 'off',
        'eqeqeq': 'off',
        'max-params': 'off'
    },
    parserOptions: {
        parser: 'babel-eslint'
    }
};
