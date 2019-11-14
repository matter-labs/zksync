import { FranklinProvider } from 'franklin_lib';
import config from './env-config';
import Axios from 'axios';
import { formatEther } from 'ethers/utils';
import { readableEther } from './utils';

export class WalletDecorator {
    constructor(address, fraProvider) {
        this.address = address;
        this.fraProvider = fraProvider || new FranklinProvider(
            config.API_SERVER,
            config.CONTRACT_ADDR
        );
        this.tokensPromise = this.fraProvider.getTokens();
    }
    async getAccount() {
        return await Axios.get(`${config.API_SERVER}/api/v0.1/account/${this.address}`).then(r => r.data);
    }
    async getCommitedBalances() {
        let account = await this.getAccount();
        console.log(account);
        return Object.entries(account.commited.balances)
            .map(([tokenId, balance]) => {
                return { 
                    tokenId,
                    balance: readableEther(balance),
                    tokenName: ['ETH', 'DAI', 'FAU'][tokenId],
                };
            });
    }
    async tokenNameFromId(tokenId) {
        let token = (await this.tokensPromise)[tokenId];
        let res = token.symbol;
        if (res) return res;
        return `erc20_${tokenId}`;
    }

    async tokenFromName(tokenName) {
        let first = (await this.tokensPromise).filter(token => token.symbol == tokenName);
        if (first.length) return first[0];
        let tokenId = tokenName.slice('erc20_'.length);
        let second = this.wallet.supportedTokens.filter(token => {
            return token.id == tokenId;
        });
        return second[0];
    }

    async transactionsAsRenderableList(address, offset, limit) {
        let transactions = await this.fraProvider.getTransactionsHistory(address, offset, limit);
        let res = transactions.map(async (tx, index) => {
            let elem_id      = `history_${index}`;
            let type         = tx.tx.type || '';
            let hash         = tx.hash;
            let direction    = 
                (type == 'Deposit') || (type == 'Transfer' && tx.tx.to == this.address)
                ? 'incoming' : 'outcoming';

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

            let status_tooltip = await (async () => {
                if (tx.commited == false) return 'Nothing';
                if (hash == null) return 'hash_null';

                let receipt = await this.fraProvider.getTxReceipt(hash);
                /**
                pub struct ProverRun {
                    pub id: i32,
                    pub block_number: i64,
                    pub worker: Option<String>,
                    pub created_at: NaiveDateTime,
                    pub updated_at: NaiveDateTime,
                }
                */
                
                if (receipt == null || receipt.prover_run == null) {
                    return 'Waiting for prover..';
                }

                let prover_name = receipt.prover_run.worker;
                let started_time = receipt.prover_run.created;
                return `Is being proved since ${started_time}`;
            })();

            // here is common data to all tx types
            let data = {
                elem_id,
                type, direction,
                status, status_tooltip, row_status,
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
                            token, 
                            amount,
                            pq_id: tx.pq_id,
                            from: tx.tx.priority_op.sender,
                            to: tx.tx.priority_op.account,
                            hash: '0x' + tx.hash,
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
                            token,
                            amount,
                            from: tx.tx.from,
                            to: tx.tx.to,
                            hash: '0x' + tx.hash,
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
                            token, 
                            amount,
                            from: this.address,
                            to: tx.tx.to,
                            hash: '0x' + tx.hash,
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
                            token, amount,
                            hash: '0x' + tx.hash,
                            to: tx.tx.eth_address,
                        }),
                    };
                }
            }
        });

        return await Promise.all(res);
    }
    async getTransactions(offset, limit) {
        return await this.transactionsAsRenderableList(this.address.slice(2), offset, limit);
    }
};
