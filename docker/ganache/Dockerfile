FROM node:10.19.0-alpine3.10

COPY . /

ENV MNEMONIC="${MNEMONIC}"

RUN yarn && yarn cache clean

EXPOSE 7545

CMD ["./ganache-entry.sh"]
