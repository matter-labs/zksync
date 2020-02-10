FROM alpine
RUN apk add --no-cache curl
COPY target/x86_64-unknown-linux-musl/release/prover /bin/
COPY docker/prover/prover-entry.sh /bin/
COPY bin/.load_keys /bin/
COPY contracts/build/ /contracts/build/
RUN apk add --no-cache axel

CMD ["prover-entry.sh"]
