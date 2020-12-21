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
        'no-debugger': 'error',
        semi: 'warn',
        'no-extra-semi': 'off',
        'no-empty': 'warn',
        'spaced-comment': 'off',
        eqeqeq: 'off',
        'max-params': 'off',
        'no-eq-null': 'off',
        'no-implicit-coercion': 'off',
        'accessor-pairs': 'off',
        'no-promise-executor-return': 'off'
    },
    parserOptions: {
        parser: 'babel-eslint'
    },
    overrides: [
        {
            files: ['./contracts/test/**/*.js'],
            rules: {
                'no-invalid-this': 'off'
            }
        }
    ]
};
