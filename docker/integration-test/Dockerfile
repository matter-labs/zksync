FROM matterlabs/geth:latest as geth
FROM debian:buster-slim

WORKDIR /usr/src/zksync

# Install required dependencies
RUN apt-get update; apt-get install -y make bash git postgresql openssl libssl-dev gcc g++ curl libpq-dev pkg-config software-properties-common jq

# Install node and yarn
RUN curl -sL https://deb.nodesource.com/setup_12.x | bash -
RUN apt-get install -y nodejs
RUN npm install -g yarn

# Install Rust and required cargo packages
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN cargo install diesel_cli --no-default-features --features postgres
RUN cargo install --version=0.1.0-beta.1 sqlx-cli

# Postgresql: Enable trust authentication to allow any connections.
RUN mkdir -p /etc/postgresql/11/main
RUN echo "host all  all    0.0.0.0/0  trust" > /etc/postgresql/11/main/pg_hba.conf

# Copy geth files
RUN apt-get install -y ca-certificates
RUN mkdir -p /seed/keystore
RUN mkdir -p /var/lib/geth/data
COPY --from=geth /seed/* /seed/
COPY --from=geth /seed/keystore/* /seed/keystore/
COPY --from=geth /bin/geth-entry.sh /usr/local/bin/
RUN curl https://gethstore.blob.core.windows.net/builds/geth-linux-amd64-1.9.22-c71a7e26.tar.gz -o geth-linux-amd64-1.9.22-c71a7e26.tar.gz
RUN tar -xzf geth-linux-amd64-1.9.22-c71a7e26.tar.gz && cp ./geth-linux-amd64-1.9.22-c71a7e26/geth /usr/local/bin

# Install `solc`
RUN curl -LO https://github.com/ethereum/solidity/releases/download/v0.5.16/solc-static-linux
RUN chmod +x solc-static-linux
RUN mv solc-static-linux /usr/local/bin/solc

# Copy the setup script
COPY entrypoint.sh /usr/local/bin/

# Setup the environment
ENV ZKSYNC_HOME=/usr/src/zksync
ENV PATH="${ZKSYNC_HOME}/bin:${PATH}"
