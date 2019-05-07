# Prerequisites

## Docker

Install docker.

## Rust

Install the latest rust version (>= 1.32):

```
rustc --version
rustc 1.32.0-nightly (21f268495 2018-12-02)
```

## Diesel

```cargo install diesel_cli --no-default-features --features postgres```

## Environment

Edit the lines below and add them to your shell profile file (e.g. `~/.bash_profile`):

```
# Add path here:
export FRANKLIN_HOME=/path/to/franklin

export PATH=$PATH:$FRANKLIN_HOME/bin
export KUBECONFIG=$FRANKLIN_HOME/etc/kube/kubeconfig.yaml
complete -W "\`grep -oE '^[a-zA-Z0-9_.-]+:([^=]|$)' $FRANKLIN_HOME/Makefile | sed 's/[^a-zA-Z0-9_.-]*$//'\`" franklin

# If you're like me, uncomment:
# cd $FRANKLIN_HOME
```

## Env configuration

## First-time setup

- Start the dev environment services:
```franklin dev-up```

- Install dependencies:
```franklin yarn```

- Setup env config:
```cp etc/env/dev.env.example etc/env/dev.env```

This will show the current config:
```franklin env```

- Create `plasma` database:
```franklin db-setup```

To reset the dev environment:

- Stop services:
```franklin  dev-down```
- Remove mounted container data:
```rm -rf ./volumes```
- Repeat the setup procedure above
