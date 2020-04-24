FROM fedora:30
ENV PATH="/root/.cargo/bin:${PATH}"
RUN dnf install -y  gettext nodejs nodejs-yarn axel jq postgresql libpq-devel rust make openssl-devel git python gcc-c++ docker which && dnf clean all \
  && curl https://sh.rustup.rs -sSf | bash -s -- -y \
  && ln -s /usr/bin/nodejs-yarn /usr/bin/yarn

RUN cargo install diesel_cli --no-default-features --features postgres
RUN cargo install wasm-pack

# sccache support
RUN cargo install sccache
ENV RUSTC_WRAPPER=/root/.cargo/bin/sccache
ENV SCCACHE_DIR=/drone/src/target/rust-ci-cache/sccache
ENV CARGO_HOME=/drone/src/target/rust-ci-cache/cargo

RUN curl -LO https://storage.googleapis.com/kubernetes-release/release/`curl -s https://storage.googleapis.com/kubernetes-release/release/stable.txt`/bin/linux/amd64/kubectl
RUN chmod +x ./kubectl
RUN mv ./kubectl /usr/local/bin/kubectl

RUN curl -LO https://github.com/ethereum/solidity/releases/download/v0.5.16/solc-static-linux
RUN chmod +x solc-static-linux
RUN mv solc-static-linux /usr/local/bin/solc
