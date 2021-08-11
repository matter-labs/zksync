# Generating new circuit keys

Prover required circuit keys. You can generate them manually.

## 0. Requirements

RAM: 128 GB CPU: 32 CPUs

## 1. Preparation

Clone repository to your machine

## 2. Installing dependencies

The following code may be used to install the dependencies (assuming that you've already cloned the repo):

```bash
#!/bin/bash

apt update
apt install htop vim yarn nodejs docker docker-compose docker-compose jq postgresql-client-12 gcc libsqlite3-dev libpq-dev libmysqlclient-dev coreutils make openssl pkg-config axel -y

# Install nvm and node 14
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.38.0/install.sh | bash
export NVM_DIR="$([ -z "${XDG_CONFIG_HOME-}" ] && printf %s "${HOME}/.nvm" || printf %s "${XDG_CONFIG_HOME}/nvm")"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh" # This loads nvm
nvm install 14

# Install yarn
npm install --global yarn

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

echo 'export ZKSYNC_HOME="$HOME/zksync-dev"' >> ~/.profile
echo 'export PATH="$ZKSYNC_HOME/bin:$PATH"' >> ~/.profile
. ~/.profile

# Assuming that you cloned repository in the previous chapter
cd zksync
```

Ensure in the commands output that every dependency was correctly installed!

## 4. Generating keys

```bash
zk
zk run plonk-setup download
zk run verify-keys gen
# Note: etc/env/dev.env.example must have correct keys/DIR
zk run verify-keys pack
```

After that, generated packed key will be placed into `keys/packed` directory. You may either copy this file to your
local machine, or commit it directly to the repo.
