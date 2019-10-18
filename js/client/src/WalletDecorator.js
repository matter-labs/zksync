import { BigNumberish, BigNumber, bigNumberify, Interface } from 'ethers/utils';
import { Contract } from 'ethers';
import { FranklinProvider, Wallet, Address } from 'franklin_lib';
import { readableEther, sleep, isReadablyPrintable } from './utils';
import timeConstants from './timeConstants';
import IERC20Conract from '../../franklin_lib/abi/IERC20.json';
import config from './env-config';

import priority_queue_abi from '../../../contracts/build/PriorityQueue.json'

function combineMessages(...args) {
    return Object.assign({}, ...args);
}

function info(msg, countdown) {
    let displayMessage = {
        kind: 'display message',
        message: msg,
        error: false,
        countdown,
    };
    return { displayMessage };
}

function error(msg, countdown) {
    let displayMessage = {
        kind: 'display message',
        message: msg,
        error: true,
        countdown,
    };
    return { displayMessage };
};

function start_progress_bar(kwargs) {
    kwargs.kind = 'start progress bar';
    return {
        startProgressBar: kwargs,
    };
}

function stop_progress_bar(kwargs) {
    let stopProgressBar = kwargs || {};
    stopProgressBar.kind = 'stop progress bar';
    return { stopProgressBar };
}

function shortenedTxHash(tx_hash) {
    return `<code class="clickable copyable" data-clipboard-text="${tx_hash}">
                ${ tx_hash.substr(0, 10) }
            </code>`;
}

export class WalletDecorator {
    // #region everything
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
    // #endregion

    // #region renderable
    async transactionsAsNeeded(offset, limit) {
        let transactions = await this.wallet.provider.getTransactionsHistory(this.wallet.address, offset, limit);
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
                : tx.fail_reason  ? `<span style="color: red">Failed with ${fail_reason}</span>`
                : `<span style="color: red">(Unknown status)</span>`

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
                    let amount = isReadablyPrintable(token)
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
                    let amount = isReadablyPrintable(token)
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
                    let amount = isReadablyPrintable(token)
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
                    let amount = isReadablyPrintable(token)
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
    async pendingDepositsAsRenderableList() {
        let tokens = await this.wallet.provider.getTokens();
        tokens.shift(); // skip ETH
        let allowances = tokens.map(async token => {
            let erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.wallet.ethWallet);
            let allowance = await erc20DeployedToken.allowance(this.ethAddress, config.CONTRACT_ADDR);
            return {
                token,
                allowance: allowance.toString(),
            };
        });
        allowances = await Promise.all(allowances);
        
        return allowances
            .filter(a => a.allowance != '0')
            .map((entry, i) => {
                entry.allowanceRenderable = readableEther(entry.allowance);
                entry.elem_id = `pendingDeposit_${i}`;
                return entry;
            });
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
            val.verified        = val.verifiedAmount  == val.committedAmount;
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
    async completeDeposit(token, amount) {
        await this.wallet.depositApprovedERC20(token, amount);
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

        yield * this.verboseGetFranklinOpStatus(res.hash);
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
            }
            
            yield info(`Sent withdraw tx to Franklin server`);
            yield * this.verboseGetFranklinOpStatus(res.hash);
    
            let tx_hash = await this.wallet.widthdrawOnchain(token, amount);

            yield * this.verboseGetRevertReason(tx_hash);
    
            yield info(`Withdraw succeeded!`);
        } catch (e) {
            yield error('Withdraw failed with ', e.message);
        }
    }

    async * verboseGetFranklinOpStatus(tx_hash) {
        let tx_hash_html = shortenedTxHash(tx_hash);

        let receipt = await this.wallet.waitTxReceipt(tx_hash);

        if (receipt.fail_reason) {
            yield error(`Transaction ${tx_hash_html} failed with <code>${receipt.fail_reason}</code>`);
            return;
        } 
        
        yield combineMessages(
            info(`Transaction ${tx_hash_html} got included in block <code>${receipt.block_number}</code>, waiting for prover...`),
            start_progress_bar({variant: 'half', duration: timeConstants.waitingForProverHalfLife})
        );

        while ( ! receipt.prover_run) {
            receipt = await this.wallet.provider.getTxReceipt(tx_hash);
            await sleep(2000);
        }

        yield combineMessages(
            info(`Prover <code>${receipt.prover_run.worker}</code> started `
                + `proving block <code>${receipt.prover_run.block_number}</code> `
                + `at <code>${receipt.prover_run.created_at}</code>`),
            start_progress_bar({variant: 'half', duration: timeConstants.provingHalfLife}) 
        );

        while ( ! receipt.verified) {
            receipt = await this.wallet.provider.getTxReceipt(tx_hash);
            await sleep(2000);
        }

        yield combineMessages(
            info(`Transaction ${tx_hash_html} got proved!`, 10),
            stop_progress_bar()
        );
        return;
    }

    async * verboseDeposit(kwargs) {
        yield info(`Sending deposit...`);

        try {
            var token = this.tokenFromName(kwargs.token);
            var amount = bigNumberify(kwargs.amount);
            this.wallet.approveERC20(token, amount); // no need to await
            var tx_hash = await this.wallet.depositApprovedERC20(token, amount);
        } catch (e) {
            yield error(`Onchain deposit failed with "${e.message}"`);
            return;
        }

        const tx_hash_html = shortenedTxHash(tx_hash);
        yield info(`Deposit ${tx_hash_html} sent to Mainchain...`);

        try {
            yield * await this.verboseGetRevertReason(tx_hash);
        } catch (e) {
            yield error(`Onchain deposit failed with "${e.message}"`);
            return;
        }

        yield * this.verboseGetPriorityOpStatus(tx_hash);
    }

    async * verboseGetPriorityOpStatus(tx_hash) {
        let priorityQueueInterface = new Interface(priority_queue_abi.interface);
        let receipt = await this.wallet.ethWallet.provider.getTransactionReceipt(tx_hash);
        let pq_id = receipt.logs
            .map(l => priorityQueueInterface.parseLog(l)) // the only way to get it to work
            .filter(Boolean)
            .filter(log => log.name == 'NewPriorityRequest');

        if (pq_id.length == 1) {
            pq_id = pq_id[0].values[0].toString();
            yield combineMessages(
                info(`Priority operation id is <code>${pq_id}</code>. Waiting for prover..`),
                start_progress_bar({variant: 'half', duration: timeConstants.waitingForProverHalfLife})
            );
        } else {
            yield error(`Found ${pq_id.length} PQ ids.`);
            return;
        }

        let pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);
        while (pq_op.prover_run == undefined) {
            await sleep(2000);
            pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);
        }

        yield combineMessages(
            info(`Prover <code>${pq_op.prover_run.worker}</code> started `
                + `proving block <code>${pq_op.prover_run.block_number}</code> `
                + `at <code>${pq_op.prover_run.created_at}</code>`),
            start_progress_bar({variant: 'half', duration: timeConstants.provingHalfLife}) 
        );

        while ( ! pq_op.verified) {
            pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);
            await sleep(2000);
        }
        
        yield combineMessages(
            info (`Priority op <code>${pq_id}</code> got proved!`, 10),
            stop_progress_bar()
        );
        return;
    }

    async * verboseGetRevertReason(tx_hash) {
        const tx_hash_html = shortenedTxHash(tx_hash);
        yield combineMessages(
            info(`Waiting for transaction ${tx_hash_html} to mine...`),
            start_progress_bar({variant: 'half', duration: timeConstants.ethereumMiningHalfLife})
        );

        let receipt;
        while (true) {
            receipt = await this.wallet.ethWallet.provider.getTransactionReceipt(tx_hash);
            
            if (receipt) break;
            await sleep(timeConstants.ethereumReceiptRetry);
        }

        if (receipt.status) {
            yield combineMessages(
                info(`Transaction ${tx_hash_html} succeeded.`),
                stop_progress_bar()
            );
        } else {
            const tx = await this.wallet.ethWallet.provider.getTransaction(tx_hash);
            const code = await this.wallet.ethWallet.provider.call(tx, tx.blockNumber);

            if (code == '0x') {
                yield error(`Transaction ${tx_hash_html} failed with empty revert reason.`);
            } else {
                const reason = code
                    .substr(138)
                    .match(/../g)
                    .map(h => parseInt(h, 16))
                    .map(String.fromCharCode)
                    .join('');
                yield error(`Transaction ${tx_hash_html} failed with <code>${reason}<code>.`);
            }
        }
    }

    // #endregion
}
