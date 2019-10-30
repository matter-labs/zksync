FROM python:3.8.0a4-alpine

RUN apk add --no-cache curl

# from https://github.com/ethereum/solidity/releases/tag/v0.5.10
RUN curl -sL https://github.com/ethereum/solidity/releases/download/v0.5.10/solc-static-linux > /bin/solc
RUN chmod a+x /bin/solc

RUN pip install solidity-flattener

ENTRYPOINT [ "sh" ]
