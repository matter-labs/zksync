FROM rust:1.67 AS builder

ENV ZKSYNC_HOME=/usr/src/zksync
WORKDIR $ZKSYNC_HOME

RUN apt update && apt install -y wget openssl libssl-dev pkg-config npm curl libpq5 libpq-dev lsb-release \
    && sh -c 'echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list' \
    && wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add - \
    && apt update && apt install -y postgresql-12 \
    && curl -sL https://deb.nodesource.com/setup_14.x | bash \
    && apt install -y nodejs \
    && npm install -g yarn

RUN cargo install diesel_cli --no-default-features --features postgres

COPY . .

RUN cargo build --release --bin zksync_data_restore

FROM debian:bullseye

ENV ZKSYNC_HOME=/usr/src/zksync

RUN apt update && apt install -y libpq5 openssl curl && apt-get clean && rm -rf /var/lib/apt/lists/*

COPY --from=builder $ZKSYNC_HOME/target/release/zksync_data_restore $ZKSYNC_HOME/bin/
COPY docker/exit-tool/configs /usr/src/configs
COPY docker/data-restore/data-restore-entry.sh /bin/

ENV PATH="${ZKSYNC_HOME}/bin:${PATH}"
ENV CONFIG_PATH=/usr/src/configs
ENV PG_DUMP_PATH=/pg_restore

RUN chmod +x /bin/data-restore-entry.sh

USER root

ENTRYPOINT ["data-restore-entry.sh"]
