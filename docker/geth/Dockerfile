FROM ethereum/client-go

RUN mkdir -p /seed/keystore
COPY password.sec /seed/
COPY fast-dev.json /seed/
COPY standard-dev.json /seed/
COPY mainnet-dev.json /seed/
COPY keystore/UTC--2019-04-06T21-13-27.692266000Z--8a91dc2d28b689474298d91899f0c1baf62cb85b /seed/keystore/

COPY geth-entry.sh /bin/

EXPOSE 8545 8546 30303 30303/udp
ENTRYPOINT [ "sh", "/bin/geth-entry.sh" ]