module.exports = {
  "extends": "solhint:recommended",
  "rules": {
    "not-rely-on-time": "off",
    "avoid-low-level-calls": "off",
    "no-inline-assembly": "off",
    "func-visibility": ["warn", {"ignoreConstructors": true}],
    "compiler-version": ["warn", "^0.7.0"]
  }
};
