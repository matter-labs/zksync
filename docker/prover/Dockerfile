FROM alpine
RUN apk add --no-cache curl
ARG BINARY[=target/x86_64-unknown-linux-musl/release/prover]
COPY $BINARY /bin/
COPY docker/prover/prover-entry.sh /bin/
COPY bin/.load_keys /bin/
RUN apk add --no-cache axel
CMD ["prover-entry.sh"]
