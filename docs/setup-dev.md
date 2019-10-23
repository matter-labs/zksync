# Prerequisites

## Docker

Install docker.

## Node & Yarn

Install Node.

Install yarn.

`yarn global add @vue/cli-service`

## Axel

Install axel for downloading keys:

```brew install axel```

## gnu-sed for MAC

`brew install gnu-sed`

## Envsubst for mac (to transpile k8s yaml files)

```
brew install gettext
brew link --force gettext 
```

## Rust

Install the latest rust version (>= 1.32) https://www.rust-lang.org/tools/install:

```
rustc --version
rustc 1.32.0-nightly (21f268495 2018-12-02)
```

# JQ

jq is used to work with json when managing DigitalOcean.

```brew install jq```

# envsubst

```bash
brew install gettext
brew link --force gettext 
```

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

# If you're like me, uncomment:
# cd $FRANKLIN_HOME
```

Autocomplete configuration.

`bash`:
```
complete -W "\`grep -oE '^[a-zA-Z0-9_.-]+:([^=]|$)' $FRANKLIN_HOME/Makefile | sed 's/[^a-zA-Z0-9_.-]*$//'\`" franklin
```

`zsh`:
```
echo "fpath=(~/.zsh_comp $fpath)" >> ~/.zshrc

mkdir -p ~/.zsh_comp
```
add `~/.zsh_comp/_franklin`:
```
#compdef franklin

cmds=( ${(uf)"$(grep -oE '^[a-zA-Z0-9_.-]+:([^=]|$)' $FRANKLIN_HOME/Makefile | sed 's/[^a-zA-Z0-9_.-]*$//')"} )

_describe 'franklin make cmds' cmds
```

