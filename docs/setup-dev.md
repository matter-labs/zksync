## Rust

Install the latest rust version (>= 1.32):

```
rustc --version
rustc 1.32.0-nightly (21f268495 2018-12-02)
```

## Makefile

For autocomplete, add this to your `~/.bash_profile`:

```
complete -W "\`grep -oE '^[a-zA-Z0-9_.-]+:([^=]|$)' Makefile | sed 's/[^a-zA-Z0-9_.-]*$//'\`" make
```

## Local geth

1. Follow the instruction here: https://hackernoon.com/hands-on-creating-your-own-local-private-geth-node-beginner-friendly-3d45902cc612
2. However, set the gaslimit to 8M *before* starting the geth for the first time!
