# syntax=docker/dockerfile:experimental
FROM debian:buster-slim

WORKDIR /usr/src/zksync

# Prepare dependencies
RUN apt-get update && apt-get install -y bash git nodejs npm postgresql openssl libssl-dev gcc g++ curl libpq-dev pkg-config software-properties-common axel
RUN apt-get install -y libpq5 ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g yarn

# Install Rust and required cargo packages
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN cargo install diesel_cli --no-default-features --features postgres
RUN cargo install --version=0.1.0-beta.1 sqlx-cli

# Copy workspace
COPY . .

# Copy data-restore configuration files
COPY docker/exit-tool/configs /usr/src/configs

# Build all the required zkSync binaries
RUN cargo build --release
RUN cargo build --release --example generate_exit_proof

COPY docker/exit-tool/exit-tool-entry.sh /usr/local/bin/

# Setup the environment
ENV ZKSYNC_HOME=/usr/src/zksync
ENV PATH="${ZKSYNC_HOME}/bin:${PATH}"
