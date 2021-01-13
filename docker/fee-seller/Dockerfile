FROM node:12
WORKDIR /usr/src/fee-seller
COPY infrastructure/fee-seller/package.json .
COPY infrastructure/fee-seller/yarn.lock .
RUN yarn install --frozen-lockfile

COPY infrastructure/fee-seller/ .

# required env
# ENV MISC_FEE_ACCOUNT_PRIVATE_KEY
# ENV MISC_MAX_LIQUIDATION_FEE_PERCENT
# ENV CHAIN_STATE_KEEPER_FEE_ACCOUNT_ADDR
# ENV CHAIN_ETH_NETWORK
# ENV ETH_CLIENT_WEB3_URL
# ENV NOTIFICATION_WEBHOOK_URL // What is it?
# optional env
# ENV MAX_LIQUIDATION_FEE_SLIPPAGE // What is it?
# ENV ETH_TRANSFER_THRESHOLD // What is it?
CMD ["yarn", "start"]
