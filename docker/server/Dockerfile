# syntax=docker/dockerfile:experimental
FROM rust:1.57 as builder
WORKDIR /usr/src/zksync
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM debian:bullseye
RUN apt-get update && apt-get install -y libpq5 ca-certificates && rm -rf /var/lib/apt/lists/*
EXPOSE 3000
EXPOSE 3031
EXPOSE 3030
EXPOSE 3002
COPY --from=builder /usr/src/zksync/target/release/zksync_server /usr/bin
COPY contracts/artifacts/ /contracts/artifacts/
COPY etc/web3-abi/ /etc/web3-abi/
ENTRYPOINT ["zksync_server"]
