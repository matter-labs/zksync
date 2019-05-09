FROM alpine
RUN apk add --no-cache curl
COPY target/x86_64-unknown-linux-musl/release/prover /
COPY docker/prover/prover-entry.sh /bin/
COPY bin/.load_keys /bin/

CMD ["prover-entry.sh"]
