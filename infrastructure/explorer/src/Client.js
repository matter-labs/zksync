import config from './env-config';
import * as constants from './constants';
import { formatToken, isBlockVerified } from './utils';
import { BlockExplorerClient } from './BlockExplorerClient';

import { Provider } from 'zksync';

import axios from 'axios';
import * as ethers from 'ethers';

import Cacher from './Cacher';

async function fetch(req) {
    let r = await axios(req);
    if (r.status == 200) {
        return r.data;
    } else {
        return null;
    }
}

function baseUrl() {
    return config.API_SERVER + '/api/v0.1';
}
export class Client {
    constructor(props) {
        Object.assign(this, props);
    }

    static async new() {
        window.syncProvider = await Provider.newHttpProvider(config.HTTP_RPC_API_ADDR);
        const tokensPromise = window.syncProvider.getTokens().then((tokens) => {
            const res = {};
            for (const token of Object.values(tokens)) {
                const symbol = token.symbol || `${token.id.toString().padStart(3, '0')}`;
                const syncSymbol = `${symbol}`;
                res[token.id] = {
                    ...token,
                    symbol,
                    syncSymbol
                };
            }
            return res;
        });
        const blockExplorerClient = new BlockExplorerClient(config.API_SERVER);
        const ethersProvider =
            config.ETH_NETWORK == 'localhost'
                ? new ethers.providers.JsonRpcProvider('http://localhost:8545')
                : ethers.getDefaultProvider();

        const props = {
            blockExplorerClient,
            tokensPromise,
            ethersProvider,
            syncProvider: window.syncProvider
        };

        // Clear the localStorage since it could have been saved before
        // But now localStorage is not used
        localStorage.clear();
        const client = new Client(props);
        const cacher = new Cacher(client);
        client.cacher = cacher;
        return client;
    }

    async getNumConfirmationsToWait(txEthBlock) {
        const numConfirmations = await this.syncProvider.getConfirmationsForEthOpAmount();
        const currBlock = await this.ethersProvider.getBlockNumber();

        return numConfirmations - (currBlock - txEthBlock);
    }

    async testnetConfig() {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/testnet_config`
        });
    }

    async status() {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/status`
        });
    }

    async loadBlocks(max_block) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/blocks?max_block=${max_block}&limit=${constants.PAGE_SIZE}`
        });
    }

    async getBlock(blockNumber) {
        const cached = this.cacher.getCachedBlock(blockNumber);
        if (cached) {
            return cached;
        }

        const block = await fetch({
            method: 'get',
            url: `${baseUrl()}/blocks/${blockNumber}`
        });

        // Cache only verified blocks since
        // these will definitely never change again
        if (isBlockVerified(block)) {
            this.cacher.cacheBlock(blockNumber, block);
        }

        return block;
    }

    async getBlockTransactions(blockNumber, blockInfo) {
        const cached = this.cacher.getCachedBlockTransactions(blockNumber);
        if (cached) {
            return cached;
        }

        let txs = await fetch({
            method: 'get',
            url: `${baseUrl()}/blocks/${blockNumber}/transactions`
        });

        // We can cache only verified transactions
        if (blockInfo && isBlockVerified(blockInfo)) {
            this.cacher.cacheBlockTransactions(blockNumber, txs);
            this.cacher.cacheTransactionsFromBlock(txs, this);
        }

        return txs;
    }

    searchBlock(query) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/search?query=${query}`
        });
    }

    async searchTx(txHash) {
        const cached = this.cacher.getCachedTransaction(txHash);
        if (cached) {
            return cached;
        }

        const tx = await fetch({
            method: 'get',
            url: `${baseUrl()}/transactions_all/${txHash}`
        });

        return tx;
    }

    async withdrawalTxHash(syncTxHash) {
        return await window.syncProvider.getEthTxForWithdrawal(syncTxHash);
    }

    getAccount(address) {
        return window.syncProvider.getState(address);
    }

    async tokenNameFromId(tokenId) {
        return (await this.tokensPromise)[tokenId].syncSymbol;
    }

    tokenNameFromSymbol(symbol) {
        return `${symbol.toString()}`;
    }

    async transactionsList(address, offset, limit) {
        if (!address) {
            return [];
        }
        const rawTransactions = await this.blockExplorerClient.getAccountTransactions(address, offset, limit);
        const transactions = rawTransactions.filter((tx) => {
            const type = tx.tx.type || '';
            if (type == 'Deposit') {
                return tx.tx.priority_op.to.toLowerCase() == address.toLowerCase();
            } else if (type == 'Withdraw') {
                return tx.tx.from.toLowerCase() == address.toLowerCase();
            } else return true;
        });
        const res = transactions.map(async (tx) => {
            const type = tx.tx.type || '';
            const hash = tx.hash;
            const created_at = tx.created_at;
            const success = Boolean(tx.success);

            // here is common data to all tx types
            const data = {
                type,
                success,
                hash,
                created_at
            };

            switch (true) {
                case type == 'Deposit': {
                    const token = this.tokenNameFromSymbol(tx.tx.priority_op.token);
                    const amount = formatToken(tx.tx.priority_op.amount || 0, token);
                    return {
                        ...data,
                        from: tx.tx.priority_op.from,
                        to: tx.tx.priority_op.to,
                        token,
                        amount
                    };
                }
                case type == 'FullExit': {
                    const token = this.tokenNameFromSymbol(tx.tx.priority_op.token);
                    const amount = formatToken(tx.tx.withdraw_amount || 0, token);
                    return {
                        ...data,
                        from: tx.tx.priority_op.eth_address,
                        to: tx.tx.priority_op.eth_address,
                        token,
                        amount
                    };
                }
                case type == 'Transfer' || type == 'Withdraw': {
                    const token = this.tokenNameFromSymbol(tx.tx.token);
                    const amount = formatToken(tx.tx.amount || 0, token);
                    return {
                        ...data,
                        from: tx.tx.from,
                        to: tx.tx.to,
                        token,
                        amount
                    };
                }
                case type == 'ForcedExit': {
                    const token = this.tokenNameFromSymbol(tx.tx.token);
                    let amount = (await this.searchTx(hash)).amount;
                    if (amount != 'unknown amount') {
                        amount = formatToken(amount || 0, token);
                    }
                    return {
                        ...data,
                        from: tx.tx.target,
                        to: tx.tx.target,
                        token,
                        amount
                    };
                }
                case type == 'Close' || type == 'ChangePubKey': {
                    return {
                        ...data,
                        from: tx.tx.account,
                        to: ''
                    };
                }
            }
        });

        const txs = await Promise.all(res);
        return txs.filter(Boolean);
    }

    async loadTokens() {
        const [tokens, tokensAcceptableForFees] = await Promise.all([
            fetch({
                method: 'get',
                url: `${baseUrl()}/tokens`
            }),
            fetch({
                method: 'get',
                url: `${baseUrl()}/tokens_acceptable_for_fees`
            })
        ]);
        tokens.forEach((token) => {
            token.acceptableForFees =
                tokensAcceptableForFees.some((element) => element.id === token.id) || token.id === 0;
        });
        return tokens;
    }
}

export const clientPromise = Client.new();
