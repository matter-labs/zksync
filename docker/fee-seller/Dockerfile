FROM node:12
WORKDIR /usr/src/fee-seller
COPY infrastructure/fee-seller/package.json .
COPY infrastructure/fee-seller/yarn.lock .
RUN yarn install --frozen-lockfile

COPY infrastructure/fee-seller/ .

# required env
# ENV FEE_ACCOUNT_PRIVATE_KEY
# ENV MAX_LIQUIDATION_FEE_PERCENT
# ENV FEE_ACCUMULATOR_ADDRESS
# ENV ETH_NETWORK
# ENV WEB3_URL
# ENV NOTIFICATION_WEBHOOK_URL
# optional env
# ENV MAX_LIQUIDATION_FEE_SLIPPAGE
# ENV ETH_TRANSFER_THRESHOLD
CMD ["yarn", "start"]
