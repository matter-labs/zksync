# syntax=docker/dockerfile:experimental
FROM rust:1.48 as builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install sccache
WORKDIR /usr/src/zksync
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/root/.cache/sccache \
    RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    cargo build --release

FROM debian:buster-slim
EXPOSE 9975
ENV RUST_LOG info
COPY --from=builder /usr/src/zksync/target/release/dev-liquidity-token-watcher /bin/
ENTRYPOINT ["dev-liquidity-token-watcher"]
