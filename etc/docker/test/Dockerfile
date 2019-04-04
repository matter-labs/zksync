# FROM rust as build

# COPY ./tmp ./
# RUN cargo build --release

FROM scratch

COPY tmp/target/x86_64-unknown-linux-musl/release/x /
ENTRYPOINT ["/x"]
