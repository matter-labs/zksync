FROM debian:buster-slim

WORKDIR /usr/src/zksync

# Install required dependencies
RUN apt-get update; apt-get install -y make bash git openssl libssl-dev gcc g++ curl libpq-dev pkg-config software-properties-common jq

# Install node and yarn
RUN curl -sL https://deb.nodesource.com/setup_14.x | bash -
RUN apt-get install -y nodejs
RUN npm install -g yarn

# Install Rust and required cargo packages
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN cargo install diesel_cli --no-default-features --features postgres
RUN cargo install --version=0.2.0 sqlx-cli

# Install `solc`
RUN curl -LO https://github.com/ethereum/solidity/releases/download/v0.5.16/solc-static-linux
RUN chmod +x solc-static-linux
RUN mv solc-static-linux /usr/local/bin/solc
RUN apt-get install -y axel postgresql

# Setup the environment
ENV ZKSYNC_HOME=/usr/src/zksync
ENV PATH="${ZKSYNC_HOME}/bin:${PATH}"
ENV CI=1
RUN cargo install sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/usr/src/cache/sccache
