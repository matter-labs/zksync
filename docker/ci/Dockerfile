FROM Docker 
ENV PATH="/root/.cargo/bin:${PATH}"
RUN dnf install -y nodejs nodejs-yarn axel jq postgresql libpq-devel rust make openssl-devel git python gcc-c++ docker && dnf clean all \
  && curl https://sh.rustup.rs -sSf | bash -s -- -y \
  && cargo install diesel_cli --no-default-features --features postgres \
  && rm -rf ~/.cargo/registry \
  && ln -s /usr/bin/nodejs-yarn /usr/bin/yarn
