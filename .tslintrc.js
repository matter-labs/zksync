// This file has nothing to do with TSLint library
// It's just ESLint config file for the typescript language

module.exports = {
    root: true,
    env: {
        browser: true,
        node: true,
        es6: true,
        mocha: true
    },
    parser: '@typescript-eslint/parser',
    plugins: ['@typescript-eslint'],
    rules: {
        // This is the only rule that should be enforced in typescript
        '@typescript-eslint/no-unused-vars': 'error'
    }
};
