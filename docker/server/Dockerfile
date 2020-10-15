# syntax=docker/dockerfile:experimental
FROM rust:1.45 as builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install sccache
WORKDIR /usr/src/zksync
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/root/.cache/sccache \
    RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    cargo build --release

FROM debian:buster-slim
RUN apt-get update && apt-get install -y libpq5 ca-certificates && rm -rf /var/lib/apt/lists/*
EXPOSE 3000
EXPOSE 3031
EXPOSE 3030
COPY --from=builder /usr/src/zksync/target/release/zksync_server /usr/bin
COPY contracts/build/ /contracts/build/
COPY bin/kube-is-leader.sh /bin/
ENTRYPOINT ["zksync_server"]
