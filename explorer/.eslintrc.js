module.exports = {
  root: true,
  env: {
    node: true
  },
  'extends': [
    'plugin:vue/essential',
    'eslint:recommended'
  ],
  rules: {
    'no-console': 'off',
    'no-debugger': process.env.NODE_ENV === 'production' ? 'error' : 'off',
    'no-unused-vars': 'warn',
    'semi': 'warn',
    'no-extra-semi': 'off',
    'no-empty': 'warn',
  },
  parserOptions: {
    parser: 'babel-eslint'
  }
};
