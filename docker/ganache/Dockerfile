FROM node:10.19.0-alpine3.10

COPY . /

ENV TEST_MNEMONIC="${TEST_MNEMONIC}"

RUN yarn && yarn cache clean

EXPOSE 7545

CMD ["./ganache-entry.sh"]
