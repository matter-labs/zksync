FROM matterlabs/geth:latest as geth
FROM matterlabs/ci:latest

WORKDIR /usr/src/zksync

# Install required cargo packages
RUN cargo install --version=0.1.0-beta.1 sqlx-cli

# Copy geth files
RUN mkdir -p /seed/keystore
RUN mkdir -p /var/lib/geth/data
COPY --from=geth /seed/* /seed/
COPY --from=geth /seed/keystore/* /seed/keystore/
COPY --from=geth /bin/geth-entry.sh /usr/local/bin/
RUN curl https://gethstore.blob.core.windows.net/builds/geth-linux-amd64-1.9.22-c71a7e26.tar.gz -o geth-linux-amd64-1.9.22-c71a7e26.tar.gz
RUN tar -xzf geth-linux-amd64-1.9.22-c71a7e26.tar.gz && cp ./geth-linux-amd64-1.9.22-c71a7e26/geth /usr/local/bin
# RUN chmod +x /bin/geth

# Copy the setup script
COPY entrypoint.sh /usr/local/bin/

# Install and run postgresql by default
RUN dnf install postgresql-server -y
RUN systemctl enable postgresql

# Postgresql: Enable trust authentication to allow any connections.
RUN mkdir -p /etc/postgresql/11/main
RUN echo "host all  all    0.0.0.0/0  trust" > /var/lib/pgsql/data/pg_hba.conf

# Setup the environment
ENV ZKSYNC_HOME=/usr/src/zksync
ENV PATH="${ZKSYNC_HOME}/bin:${PATH}"
