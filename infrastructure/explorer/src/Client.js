import config from './env-config';
import * as constants from './constants';
import {
    formatToken
} from './utils';
import {
    BlockExplorerClient
} from './BlockExplorerClient';
const zksync_promise = import('zksync');
import axios from 'axios';
import * as ethers from 'ethers';

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
        const zksync = await zksync_promise;
        window.syncProvider = await zksync.Provider.newHttpProvider(config.HTTP_RPC_API_ADDR);
        const tokensPromise = window.syncProvider.getTokens()
            .then(tokens => {
                const res = {};
                for (const token of Object.values(tokens)) {
                    const symbol = token.symbol || `${token.id.toString().padStart(3, '0')}`;
                    const syncSymbol = `${symbol}`;
                    res[token.id] = {
                        ...token,
                        symbol,
                        syncSymbol,
                    };
                }
                return res;
            });
        const blockExplorerClient = new BlockExplorerClient(config.API_SERVER);
        const ethersProvider = config.ETH_NETWORK == 'localhost' ?
            new ethers.providers.JsonRpcProvider('http://localhost:8545') :
            ethers.getDefaultProvider();

        const props = {
            blockExplorerClient,
            tokensPromise,
            ethersProvider,
            syncProvider: window.syncProvider,
        };

        return new Client(props);
    }

    async getNumConfirmationsToWait(txEthBlock) {
        const numConfirmations = await this.syncProvider.getConfirmationsForEthOpAmount();
        const currBlock = await this.ethersProvider.getBlockNumber();

        return numConfirmations - (currBlock - txEthBlock);
    }

    async testnetConfig() {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/testnet_config`,
        });
    }

    async status() {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/status`,
        });
    }

    async loadBlocks(max_block) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/blocks?max_block=${max_block}&limit=${constants.PAGE_SIZE}`,
        });
    }

    async getBlock(blockNumber) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/blocks/${blockNumber}`,
        });
    }

    async getBlockTransactions(blockNumber) {
        let txs = await fetch({
            method: 'get',
            url: `${baseUrl()}/blocks/${blockNumber}/transactions`,
        });

        return txs;
    }

    searchBlock(query) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/search?query=${query}`,
        });
    }

    searchTx(txHash) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/transactions_all/${txHash}`,
        });
    }

    getAccount(address) {
        return window.syncProvider.getState(address);
    }

    async getCommitedBalances(address) {
        const account = await this.getAccount(address);

        let balances = Object.entries(account.committed.balances)
            .map(([tokenSymbol, balance]) => {
                return {
                    tokenSymbol,
                    balance: formatToken(balance, tokenSymbol),
                };
            });

        balances.sort((a, b) => a.tokenSymbol.localeCompare(b.tokenSymbol));
        return balances;
    }

    async tokenNameFromId(tokenId) {
        return (await this.tokensPromise)[tokenId].syncSymbol;
    }

    tokenNameFromSymbol(symbol) {
        return `${symbol.toString()}`;
    }

    async transactionsList(address, offset, limit) {
        if (!address) {
            console.log(address);
            return [];
        }
        const transactions = await this.blockExplorerClient.getAccountTransactions(address, offset, limit);
        const res = transactions.map(async (tx, index) => {
            const type = tx.tx.type || '';
            const hash = tx.hash;
            const created_at = tx.created_at;
            const success = Boolean(tx.success);

            // here is common data to all tx types
            const data = {
                type,
                success,
                hash,
                created_at,
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
                        amount,
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
                        amount,

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
                        amount,
                    };
                }
                case type == 'ForcedExit': {
                    const token = this.tokenNameFromSymbol(tx.tx.token);
                    let amount = (await this.searchTx(hash)).amount;
                    if (amount != "unknown amount") {
                        amount = formatToken(amount || 0, token);
                    }
                    return {
                        ...data,
                        from: tx.tx.target,
                        to: tx.tx.target,
                        token,
                        amount,
                    };
                }
                case type == 'Close' || type == 'ChangePubKey': {
                    return {
                        ...data,
                        from: tx.tx.account,
                        to: '',
                    };
                }
            }
        });

        const txs = await Promise.all(res);
        return txs.filter(Boolean);
    }

    async loadTokens() {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/tokens`,
        });
    }
};

export const clientPromise = Client.new();