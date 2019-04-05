FROM alpine
RUN apk add --no-cache curl
COPY target/x86_64-unknown-linux-musl/release/prover /
COPY src/bin/prover-entry.sh /
CMD ["/prover-entry.sh"]
