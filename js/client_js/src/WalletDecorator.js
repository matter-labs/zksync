import { BigNumberish, BigNumber, bigNumberify, Interface } from 'ethers/utils';
import { Contract } from 'ethers';
import { FranklinProvider, Wallet, Address } from 'franklin_lib';
import { readableEther, sleep } from './utils';

// TODO:
import env_config from './env-config'
import priority_queue_abi from '../../../contracts/build/PriorityQueue.json'
import franklinContract from '../../franklin_lib/abi/Franklin.json'

function info(msg) {
    return {
        message: msg, 
        error: false,
    };
}

function error(msg) {
    return {
        message: msg,
        error: true,
    }
};

export class WalletDecorator {
    constructor (wallet) {
        this.wallet = wallet;
        this.address = '0x' + this.wallet.address.toString('hex');
    }

    static async new(wallet) {
        let res = new WalletDecorator(wallet);
        res.ethAddress = await wallet.ethWallet.getAddress();
        return res;
    }

    async getDepositFee() {
        let gasPrice = await this.wallet.ethWallet.provider.getGasPrice();
        let gasLimit = bigNumberify(200000); // from wallet.ts
        let fee = gasPrice.mul(gasLimit);
        return readableEther(fee);
    }

    async updateState() {
        await this.wallet.updateState();
        let address = this.wallet.address;
    }

    tokenNameFromId(tokenId) {
        let token = this.wallet.supportedTokens[tokenId];
        let res = token.symbol;
        if (res) return res;
        return `erc20_${tokenId}`;
    }

    tokenFromName(tokenName) {
        let first = this.wallet.supportedTokens.filter(token => token.symbol == tokenName);
        if (first.length) return first[0];
        let tokenId = tokenName.slice('erc20_'.length);
        let second = this.wallet.supportedTokens.filter(token => {
            return token.id == tokenId;
        });
        return second[0];
    }

    // #region renderable
    async transactionsAsNeeded() {
        let transactions = await this.wallet.provider.getTransactionsHistory(this.wallet.address);
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
                : `<span style="color: red">(WTF)</span>`;

            let row_status
                = tx.verified     ? `<span style="color: green">Verified</span>`
                : tx.commited     ? `<span style="color: grey">Committed</span>`
                : tx.fail_reason  ? `<span style="color: red">Failed with ${fail_reason}</span>`
                : `<span style="color: red">Not committed, not verified, no fail reason.</span>`

            let status_tooltip = await (async () => {
                if (tx.commited == false) return 'Nothing';
                if (hash == null) return 'hash_null';

                let receipt = await this.wallet.provider.getTxReceipt(hash);
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
                    let token = this.tokenNameFromId(tx.tx.priority_op.token);
                    let amount = token == 'ETH' 
                        ? readableEther(tx.tx.priority_op.amount) 
                        : bigNumberify(tx.tx.priority_op.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'pq_id',       label: 'Priority op' },
                        ],
                        data: Object.assign(data, {
                            pq_id: tx.pq_id,
                            token, amount,
                        }),
                    };
                }
                case type == 'Transfer' && direction == 'incoming': {
                    let token = this.tokenNameFromId(tx.tx.token);
                    let amount = token == 'ETH' 
                        ? readableEther(tx.tx.amount) 
                        : bigNumberify(tx.tx.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'from',        label: 'From' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: Object.assign(data, {
                            from: tx.tx.from,
                            token, amount,
                            hash: tx.hash,
                        }),                    
                    }
                }
                case type == 'Transfer' && direction == 'outcoming': {
                    let token = this.tokenNameFromId(tx.tx.token);
                    let amount = token == 'ETH' 
                        ? readableEther(tx.tx.amount) 
                        : bigNumberify(tx.tx.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'to',          label: 'To' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: Object.assign(data, {
                            to: tx.tx.to,
                            token, amount,
                            hash: tx.hash,
                        }),
                    }
                }
                case type == 'Withdraw': {
                    let token = this.tokenNameFromId(tx.tx.token);
                    let amount = token == 'ETH' 
                        ? readableEther(tx.tx.amount) 
                        : bigNumberify(tx.tx.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: Object.assign(data, {
                            token, amount,
                            hash: tx.hash,
                        }),
                    }
                }
            }
        });

        return await Promise.all(res);
    }

    onchainBalancesAsRenderableList() {
        return this.wallet.ethState.onchainBalances
            .map((balance, tokenId) => ({
                tokenName: this.tokenNameFromId(tokenId),
                address: this.wallet.supportedTokens[tokenId].address,
                amount: balance.toString()
            }))
            .filter(tokenInfo => Number(tokenInfo.amount));
    }
    contractBalancesAsRenderableList() {
        return this.wallet.ethState.contractBalances
            .map((balance, tokenId) => ({
                tokenName: this.tokenNameFromId(tokenId),
                address: this.wallet.supportedTokens[tokenId].address,
                amount: `${balance.toString()}`
            }))
            .filter(tokenInfo => Number(tokenInfo.amount));
    }
    franklinBalancesAsRenderableListWithInfo() {
        let res = {};
        let assign = key => entry => {
            let [tokenId, balance] = entry;
            if (res[tokenId] === undefined) {
                res[tokenId] = {
                    tokenName: this.tokenNameFromId(tokenId),
                    address: this.wallet.supportedTokens[tokenId].address,
                };
            }
            res[tokenId][key] = balance;
        };
        Object.entries(this.wallet.franklinState.commited.balances).forEach(assign('committedAmount'))
        Object.entries(this.wallet.franklinState.verified.balances).forEach(assign('verifiedAmount'))
        return Object.values(res).map(val => {
            val['committedAmount'] = val['committedAmount'] || bigNumberify(0);
            val['verifiedAmount']  = val['verifiedAmount']  || bigNumberify(0);
            return val;
        }).filter(entry => Number(entry.committedAmount) || Number(entry.verifiedAmount));
    }
    franklinBalancesAsRenderableList() {
        return Object.entries(this.wallet.franklinState.commited.balances)
            .map(entry => {
                let [tokenId, balance] = entry;
                return {
                    tokenName: this.tokenNameFromId(tokenId),
                    amount: balance
                };
            }).filter(bal => Number(bal.amount));
    }
    // #endregion

    // #region actions
    async transfer(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);
        
        let res = await this.wallet.transfer(kwargs.address, token, amount, fee);

        if (res.err) throw new Error(res.err);
        let receipt = await this.wallet.waitTxReceipt(res.hash);
        if (receipt.fail_reason) throw new Error(receipt.fail_reason);
    }

    async * verboseTransfer(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);
        
        try {
            var res = await this.wallet.transfer(kwargs.address, token, amount, fee);
        } catch (e) {
            yield error(`Sending transfer failed with ${e.message}`);
            return;
        }

        if (res.err == null) {
            yield info(`Sent transfer to Matters server`);
        } else {
            yield error(`Transfer failed with ${res.err}`);
            return;
        }

        let receipt = await this.wallet.waitTxReceipt(res.hash);

        if (receipt.fail_reason) {
            yield error(`Transaction failed with ${receipt.fail_reason}`);
        } else {
            yield info(`Tx <code>${res.hash}</code> got included in block ${receipt.block_number}, waiting for prover`);
        }

        while ( ! receipt.prover_run) {
            receipt = await this.wallet.waitTxReceipt(res.hash)
            await sleep(1000);
        }

        yield info(`Prover ${receipt.prover_run.worker} started `
            + `proving block ${receipt.prover_run.block_number} `
            + `at ${receipt.prover_run.created_at}`);

        await sleep(3000);

        while ( ! receipt.verified) {
            receipt = await this.wallet.waitTxReceipt(res.hash)
            await sleep(1000);
        }

        yield info (`Transfer <code>${res.hash}</code> got proved!`);
        await sleep(5000);
        return;
    }

    async * verboseWithdraw(kwargs) {
        try {

            yield info(`Sending withdraw...`);
    
            let token = this.tokenFromName(kwargs.token);
            let amount = bigNumberify(kwargs.amount);
            let fee = bigNumberify(kwargs.fee);
    
            let res = await this.wallet.widthdrawOffchain(token, amount, fee);
    
            if (res.err) {
                yield error(`Offchain withdraw failed with ${res.err}`);
                return;
            } else {
                yield info(`Sent withdraw tx to Franklin server`);
            }
        
            yield info(`Getting receipt...`);
    
            try {
                var receipt = await this.wallet.waitTxReceipt(res.hash);
            } catch (e) {
                yield error(`Failed to get the receipt with ${e.message}`);
                return;  
            }
    
            if (receipt.fail_reason) {
                yield error(`Transaction failed with ${receipt.fail_reason}`);
                return;
            } else {
                yield info(`Tx <code>${res.hash}</code> got included in block ${receipt.block_number}, waiting for prover`);
            }
    
            while ( ! receipt.prover_run) {
                receipt = await this.wallet.waitTxReceipt(res.hash)
                await sleep(1000);
            }
    
            yield info(`Prover ${receipt.prover_run.worker} started `
                + `proving block ${receipt.prover_run.block_number} `
                + `at ${receipt.prover_run.created_at}`);
    
            await sleep(3000);
    
            while ( ! receipt.verified) {
                receipt = await this.wallet.waitTxReceipt(res.hash)
                await sleep(1000);
            }
    
            yield info(`Tx <code>${res.hash}</code> got proved! Starting onchain withdraw...`);
    
            await sleep(5000);
    
            var tx_hash = await this.wallet.widthdrawOnchain(token, amount);

            yield * this.verboseGetRevertReason(tx_hash);
    
            yield info(`Withdraw succeeded!`);
            await sleep(5000);
        } catch (e) {
            yield error('Withdraw failed with ', e.message);
            await sleep(5000);
        }
    }

    async depositOnchain(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);
        let fullAmount = amount.add(fee);
        let tx_hash = await this.wallet.depositOnchain(token, fullAmount);
        return tx_hash;
    }

    async depositOffchain(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);

        let res = await this.wallet.depositOffchain(token, amount, fee);

        if (res.err) {
            throw new Error(res.err);
        }

        let receipt = await this.wallet.waitTxReceipt(res.hash);

        if (receipt.fail_reason) {
            throw new Error(receipt.fail_reason);
        }
        return 0;
    }

    async * verboseDeposit(kwargs) {
        yield info(`Sending deposit...`);

        try {
            var token = this.tokenFromName(kwargs.token);
            var amount = bigNumberify(kwargs.amount);
            var tx_hash = await this.wallet.deposit(token, amount);
            yield info(`Deposit <code>${tx_hash}</code> sent to Mainchain...`);
        } catch (e) {
            yield error(`Onchain deposit failed with "${e.message}"`);
            await sleep(5000)
            return;
        }

        try {
            yield * await this.verboseGetRevertReason(tx_hash);
            await sleep(5000);
        } catch (e) {
            yield error(`Onchain deposit failed with "${e.message}"`);
            await sleep(5000)
            return;
        }

        let priorityQueueInterface = new Interface(priority_queue_abi.interface);
        let receipt = await this.wallet.ethWallet.provider.getTransactionReceipt(tx_hash);
        let pq_id = receipt.logs
            .map(l => priorityQueueInterface.parseLog(l)) // the only way to get it to work
            .filter(Boolean)
            .filter(log => log.name == 'NewPriorityRequest')

        if (pq_id.length == 1) {
            pq_id = pq_id[0].values[0].toString();
            yield info(`PQ id is ${pq_id}`);
        } else {
            console.log('pq_id : ', pq_id);
            yield error(`Found ${pq_id.length} PQ ids.`);
            return;
        }

        let pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);
        while (pq_op.prover_run == undefined) {
            await sleep(2000);
            pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);
            console.log(pq_op);
        }

        yield info(`Prover ${pq_op.prover_run.worker} started `
            + `proving block ${pq_op.prover_run.block_number} `
            + `at ${pq_op.prover_run.created_at}`);

        while ( ! pq_op.verified) {
            pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);
            await sleep(2000);
        }
        
        yield info (`PQ op <code>${pq_id}</code> got proved!`);
        await sleep(5000);
        return;
    }

    async * verboseGetRevertReason(tx_hash) {
        let receipt;
        for (let i = 1; i <= 5 && !receipt; i++) {
            yield info(`Getting tx receipt...`);
            
            receipt = await this.wallet.ethWallet.provider.getTransactionReceipt(tx_hash);
            
            if (receipt) break;
            await sleep(4000);
        }

        if (receipt == null) {
            yield error(`Can't get tx receipt after 5 retries`);
            return;
        }

        if (receipt.status) {
            yield info(`Transaction succeeded.`);
        } else {
            const tx = await this.wallet.ethWallet.provider.getTransaction(tx_hash);
            const code = await this.wallet.ethWallet.provider.call(tx, tx.blockNumber);

            if (code == '0x') {
                yield error('Empty revert reason code');
            } else {
                const reason = code
                    .substr(138)
                    .match(/../g)
                    .map(h => parseInt(h, 16))
                    .map(String.fromCharCode)
                    .join('');
                yield error(`Revert reason is: ${reason}`);
            }
        }
    }

    // #endregion
}
