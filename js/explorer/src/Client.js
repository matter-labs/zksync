import config from './env-config';
import constants from './constants';
import { readableEther } from './utils';
import { BlockExplorerClient } from './BlockExplorerClient';
const zksync = require('zksync');   
const ethers = require('ethers');
import axios from 'axios';

async function fetch(req) {
    let r = await axios(req).catch(_ => ({}));
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
        const syncProvider = await zksync.Provider.newWebsocketProvider(config.WS_API_ADDR);
        
        const tokensPromise = syncProvider.getTokens()
            .then(tokens => {
                return Object.values(tokens)
                    .map(token => ({
                        ...token,
                        symbol: token.symbol || `${token.id.toString().padStart(3, '0')}`,
                    }))
                    .sort((a, b) => a.id - b.id);
            });
        
        const blockExplorerClient = new BlockExplorerClient(config.API_SERVER);

        const props = {
            blockExplorerClient,
            tokensPromise,
        };

        return new Client(props);
    }

    async status() {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/status`,
        });
    }

    async loadBlocks(max_block) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks?max_block=${max_block}&limit=${constants.PAGE_SIZE}`,
        });
    }

    async getBlock(blockNumber) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks/${blockNumber}`,
        });
    }

    async getBlockTransactions(blockNumber) {
        let txs = await fetch({
            method:     'get',
            url:        `${baseUrl()}/blocks/${blockNumber}/transactions`,
        });
        
        return txs.map(tx => {
            let res = tx.op;
            res.tx_hash = tx.tx_hash;
            return res;
        });
    }

    searchBlock(query) {
        return fetch({
            method:     'get',
            url:        `${baseUrl()}/search?query=${query}`,
        });
    }

    searchAccount(address) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/account/${address}`,
        });
    }
    
    searchTx(txHash) {
        return fetch({
            method: 'get',
            url: `${baseUrl()}/transactions_all/${txHash}`,
        });
    }

    getAccount(address) {
        return fetch({
            method: 'get',
            url: `${config.API_SERVER}/api/v0.1/account/${address}`,
        });
    }

    async getCommitedBalances(address) {
        const account = await this.getAccount(address);
        const tokensInfoList = await this.tokensPromise;

        return Object.entries(account.commited.balances)
            .map(([tokenId, balance]) => {
                return { 
                    tokenId,
                    balance: readableEther(balance),
                    tokenName: tokensInfoList[tokenId].symbol,
                };
            });
    }
    async tokenNameFromId(tokenId) {
        return (await this.tokensPromise)[tokenId].symbol;
    }

    async transactionsAsRenderableList(address, offset, limit) {
        if (!address) {
            console.log(address);
            return [];
        }
        let transactions = await this.blockExplorerClient.getAccountTransactions(address, offset, limit);
        let res = transactions.map(async (tx, index) => {
            let elem_id      = `history_${index}`;
            let type         = tx.tx.type || '';
            let hash         = tx.hash;
            let direction    = 
                (type == 'Deposit') || (type == 'Transfer' && tx.tx.to == address)
                ? 'incoming' 
                : 'outcoming';

            // pub hash: Option<String>,
            // pub tx: Value,
            // pub success: Option<bool>,
            // pub fail_reason: Option<String>,
            // pub commited: bool,
            // pub verified: bool,

            let status
                = tx.verified            ? `<span style="color: green">(verified)</span>`
                : tx.success == null     ? `<span style="color: grey">(pending)</span>`
                : tx.success == true     ? `<span style="color: grey">(succeeded)</span>`
                : tx.commited            ? `<span style="color: grey">(committed)</span>`
                : tx.fail_reason != null ? `<span style="color: red">(failed)</span>`
                : `<span style="color: red">(Unknown status)</span>`;

            let row_status
                = tx.verified     ? `<span style="color: green">Verified</span>`
                : tx.commited     ? `<span style="color: grey">Committed</span>`
                : tx.fail_reason  ? `<span style="color: red">Failed with ${tx.fail_reason}</span>`
                : `<span style="color: red">(Unknown status)</span>`;

            // here is common data to all tx types
            let data = {
                elem_id,
                type, direction,
                status, row_status,
            };

            switch (true) {
                case type == 'Deposit': {
                    let token = await this.tokenNameFromId(tx.tx.priority_op.token);
                    let amount = readableEther(tx.tx.priority_op.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'pq_id',       label: 'Priority op' },
                        ],
                        data: Object.assign(data, {
                            from: tx.tx.priority_op.sender,
                            to: tx.tx.priority_op.account,
                            pq_id: tx.pq_id,
                            token, amount,
                            hash,
                        }),
                    };
                }
                case type == 'Transfer' && direction == 'incoming': {
                    let token = await this.tokenNameFromId(tx.tx.token);
                    let amount = readableEther(tx.tx.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'from',        label: 'From' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: Object.assign(data, {
                            from: tx.tx.from,
                            to: tx.tx.to,
                            token, amount,
                            hash: tx.hash,
                        }),                    
                    };
                }
                case type == 'Transfer' && direction == 'outcoming': {
                    let token = await this.tokenNameFromId(tx.tx.token);
                    let amount = readableEther(tx.tx.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'to',          label: 'To' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: Object.assign(data, {
                            from: tx.tx.from,
                            to: tx.tx.to,
                            token, amount,
                            hash: tx.hash,
                        }),
                    };
                }
                case type == 'Withdraw': {
                    let token = await this.tokenNameFromId(tx.tx.token);
                    let amount = readableEther(tx.tx.amount);   
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: Object.assign(data, {
                            from: tx.tx.account,
                            to: tx.tx.ethAddress,
                            token, amount,
                            hash: tx.hash,
                        }),
                    };
                }
            }
        });

        return await Promise.all(res);
    }
};

export const clientPromise = Client.new();
