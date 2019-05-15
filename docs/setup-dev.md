# Prerequisites

## Docker

Install docker.

## Node & Yarn

Install Node.

Install yarn.

## Axel

Install axel for downloading keys:

```brew install axel```

## Rust

Install the latest rust version (>= 1.32):

```
rustc --version
rustc 1.32.0-nightly (21f268495 2018-12-02)
```

# JQ

jq is used to work with json when managing DigitalOcean.

```brew install jq```

# PSQL

Install `psql` CLI tool to interact with postgres.

## Diesel

```cargo install diesel_cli --no-default-features --features postgres```

## Environment

Edit the lines below and add them to your shell profile file (e.g. `~/.bash_profile`):

```
# Add path here:
export FRANKLIN_HOME=/path/to/franklin

export PATH=$FRANKLIN_HOME/bin:$PATH
complete -W "\`grep -oE '^[a-zA-Z0-9_.-]+:([^=]|$)' $FRANKLIN_HOME/Makefile | sed 's/[^a-zA-Z0-9_.-]*$//'\`" franklin

# If you're like me, uncomment:
# cd $FRANKLIN_HOME
```
