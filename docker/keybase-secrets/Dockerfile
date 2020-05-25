FROM keybaseio/client
ENV KEYBASE_SERVICE=1
RUN apt-get update \
    && apt-get install -y git \
    && rm -rf /var/lib/apt/lists/*
COPY docker/keybase-secrets/entrypoint.sh /
ENTRYPOINT ["/entrypoint.sh"]
