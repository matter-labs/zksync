module.exports = {
  "extends": "solhint:recommended",
  "rules": {
    // Unfortunately on the time of this writing, `--quiet` option of solhint is not working.
    // And also there were >290 warnings on *.sol files. Since changes to *.sol 
    // files require an audit, it was decided to postpone the changes to make the solhint
    // pass.
    //
    // TODO: Turn on the majority of the rules 
    // and make the solhint comply to them. (ZKS-329)
    "state-visibility": "off",
    "var-name-mixedcase": "off",
    "avoid-call-value": "off",
    "no-empty-blocks": "off",
    "not-rely-on-time": "off",
    "avoid-low-level-calls": "off",
    "no-inline-assembly": "off",
    "const-name-snakecase": "off",
    "no-complex-fallback": "off",
    "reason-string": "off",
    "func-name-mixedcase": "off",
    "no-unused-vars": "off",
    "max-states-count": "off",
    "compiler-version": ["warn", "^0.7.0"]
  }
};
