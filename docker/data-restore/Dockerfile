# syntax=docker/dockerfile:experimental
FROM rust:1.52 as builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install sccache
WORKDIR /usr/src/zksync
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/root/.cache/sccache \
    RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    cargo build --release --bin zksync_data_restore

FROM debian:buster-slim
RUN apt update && apt install wget openssl npm curl libpq5 libpq-dev lsb-release -y
# PostgreSQL Apt Repository is used to install the compatible psql version.
# Create the file repository configuration:
RUN sh -c 'echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list'
# Import the repository signing key:
RUN wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -
# Update the package lists:
RUN apt update
RUN apt install postgresql-12 -y
RUN curl -sL https://deb.nodesource.com/setup_14.x | bash
RUN apt install nodejs -y
RUN npm install -g yarn

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN cargo install diesel_cli --no-default-features --features postgres

COPY --from=builder /usr/src/zksync/ /usr/src/zksync/

# Copy configuration files for data restore.
COPY docker/exit-tool/configs /usr/src/configs
COPY docker/data-restore/data-restore-entry.sh /bin/

# Setup the environment
ENV ZKSYNC_HOME=/usr/src/zksync
ENV PATH="${ZKSYNC_HOME}/bin:${PATH}"
ENV IN_DOCKER=true

RUN cd $ZKSYNC_HOME && zk

ENTRYPOINT ["data-restore-entry.sh"]
