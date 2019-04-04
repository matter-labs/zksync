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
