import { BigNumberish, BigNumber, bigNumberify, Interface } from 'ethers/utils';
import { Contract } from 'ethers';
import { FranklinProvider, Wallet, Address } from 'franklin_lib';
import { readableEther, sleep, isReadablyPrintable } from './utils';
import timeConstants from './timeConstants';
import IERC20Conract from '../../franklin_lib/abi/IERC20.json';
import config from './env-config';

import priority_queue_abi from '../../../contracts/build/PriorityQueue.json'

const NUMERIC_LIMITS_UINT_256 = '115792089237316195423570985008687907853269984665640564039457584007913129639935';

function combineMessages(...args) {
    return Object.assign({}, ...args);
}

function info(msg, kwargs) {
    let displayMessage = {
        message: msg,
        error: false,
        variant: 'success',
        countdown: 100,
    };
    
    // update default displayMessage with values from kwargs dict.
    Object.assign(displayMessage, kwargs);

    return { displayMessage };
}

function error(msg, kwargs) {
    let displayMessage = {
        message: msg,
        error: true,
        variant: 'danger',
        countdown: 100,
    };
    
    // update default displayMessage with values from kwargs dict.
    Object.assign(displayMessage, kwargs);

    return { displayMessage };
};

function start_progress_bar(kwargs) {
    let startProgressBar = {
        variant: 'half',
        duration: timeConstants.waitingForProverHalfLife,
    };

    // update default startProgressBar with values from kwargs dict.
    Object.assign(startProgressBar, kwargs);

    return { startProgressBar };
}

function stop_progress_bar(kwargs) {
    let stopProgressBar = {};

    // update default stopProgressBar with values from kwargs dict.
    Object.assign(stopProgressBar, kwargs);

    return { stopProgressBar };
}

function shortenedTxHash(tx_hash) {
    return `<code class="clickable copyable" data-clipboard-text="${tx_hash}">
                ${ tx_hash.substr(0, 10) }
            </code>`;
}

function addPendingWithdrawOffchainOperation(kwargs, hash) {
    let already = JSON.parse(window.localStorage.getItem('pendingWithdrawOps') || '{}');
    kwargs.pendingStatus = 'not started';
    already[hash] = kwargs;
    window.localStorage.setItem('pendingWithdrawOps', JSON.stringify(already));
}

function setPendingWithdrawOffchainOperationStatus(hash, new_status) {
    let already = JSON.parse(window.localStorage.getItem('pendingWithdrawOps') || '{}');
    already[hash].pendingStatus = new_status;
    window.localStorage.setItem('pendingWithdrawOps', JSON.stringify(already));
}

function removePendingWithdrawOffchainOperation(hash) {
    let already = JSON.parse(window.localStorage.getItem('pendingWithdrawOps') || '{}');
    delete already[hash];
    window.localStorage.setItem('pendingWithdrawOps', JSON.stringify(already));
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
    async allowancesForAllTokens() {
        let tokens = await this.wallet.provider.getTokens();
        tokens.shift(); // skip ETH
        let allowances = tokens.map(async token => {
            let erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.wallet.ethWallet);
            let amount = await erc20DeployedToken.allowance(this.ethAddress, config.CONTRACT_ADDR);
            return { token, amount };
        });
        return await Promise.all(allowances);
    }
    async pendingDepositsAsRenderableList() {
        let allowances = await this.allowancesForAllTokens();
        return allowances
            .map(a => ({
                token: a.token,
                amount: a.amount.toString()
            }))
            .filter(a => a.amount != '0')
            .map((op, i) => {
                op.operation = 'Deposit';
                op.elem_id = `pendingDeposit_${i}`;
                op.amountRenderable = readableEther(op.amount);
                return op;
            });
    }
    pendingWithdrawsAsRenderableList() {
        return [window.localStorage.getItem('pendingWithdrawOps') || "{}"]
            .map(JSON.parse)
            .map(Object.entries)
            .pop()
            .map(([hash, op], i) => {
                op.hash = hash;
                op.operation = 'Withdraw';
                op.elem_id = `pendingWithdraw_${i}`;
                op.amountRenderable = readableEther(op.amount);
                return op;
            });
    }
    pendingOperationsAsRenderableList() {
        return this.pendingWithdrawsAsRenderableList();
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
        [this.wallet.franklinState.commited.balances].map(Object.entries).pop().forEach(assign('committedAmount'));
        [this.wallet.franklinState.verified.balances].map(Object.entries).pop().forEach(assign('verifiedAmount'));
        return Object.values(res).map(val => {
            val['committedAmount'] = val['committedAmount'] || bigNumberify(0);
            val['verifiedAmount']  = val['verifiedAmount']  || bigNumberify(0);
            val.verified           = val.verifiedAmount     == val.committedAmount;
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
    async waitTxMine(hash) {
        let tx;
        do {
            tx = await this.wallet.ethWallet.provider.getTransaction(hash);
        } while (tx.blockHash || await sleep(2000));
        return tx;
    }
    async completeWithdraw(token, amount, hash) {
        await this.wallet.widthdrawOnchain(token, amount);
        removePendingWithdrawOffchainOperation(hash);
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
            
            // we should know later how to finish this op, so we pass kwargs
            addPendingWithdrawOffchainOperation({ token, amount: kwargs.amount.toString() }, res.hash);
    
            if (res.err) {
                yield error(`Offchain withdraw failed with ${res.err}`);
                return;
            }
            
            yield info(`Sent withdraw tx to Franklin server`);
            yield * this.verboseGetFranklinOpStatus(res.hash);
    
            let tx = await this.wallet.widthdrawOnchain(token, amount);
            let tx_hash = tx.hash;
            removePendingWithdrawOffchainOperation(res.hash);

            await tx.wait(2);
            yield * this.verboseGetRevertReason(tx_hash);
    
            yield info(`Withdraw succeeded!`);
        } catch (e) {
            yield combineMessages(
                error('Withdraw failed with ', e.message, { timeout: 7 }),
            );
            return;
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

        let proverStart = new Date();
        
        yield combineMessages(
            info(`Prover <code>${receipt.prover_run.worker}</code> is `
                + `proving block <code>${receipt.prover_run.block_number}</code> `
                + `for <code>${Math.round((new Date() - proverStart) / 1000)}</code> seconds`),
            start_progress_bar({variant: 'half', duration: timeConstants.provingHalfLife}) 
        );

        while ( ! receipt.verified) {
            yield info(`Prover <code>${receipt.prover_run.worker}</code> is `
                + `proving block <code>${receipt.prover_run.block_number}</code> `
                + `for <code>${Math.round((new Date() - proverStart) / 1000)}</code> seconds`);

            receipt = await this.wallet.provider.getTxReceipt(tx_hash);
            await sleep(1000);
        }

        yield combineMessages(
            info(`Transaction ${tx_hash_html} got proved!`, { countdown: 10 }),
            stop_progress_bar()
        );
        return;
    }

    async * verboseDeposit(kwargs) {
        try {
            yield info(`Sending deposit...`);

            let token = this.tokenFromName(kwargs.token);
            let amount = bigNumberify(kwargs.amount);
            
            let tx_hash;
            if (kwargs.token == 'ETH') {
                tx_hash = await this.wallet.depositETH(amount);
            } else {
                let erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.wallet.ethWallet);
                let allowance = await erc20DeployedToken.allowance(this.ethAddress, config.CONTRACT_ADDR);
                if (allowance.toString().length != NUMERIC_LIMITS_UINT_256.length) {
                    await this.wallet.approveERC20(token, NUMERIC_LIMITS_UINT_256);
                }

                tx_hash = await this.wallet.depositApprovedERC20(token, amount);
            }

            const tx_hash_html = shortenedTxHash(tx_hash);
            yield info(`Deposit ${tx_hash_html} sent to Mainchain...`);

            yield * await this.verboseGetRevertReason(tx_hash);
            
            yield * this.verboseGetPriorityOpStatus(tx_hash);
        } catch (e) {
            yield combineMessages(
                error(`Onchain deposit failed with "${e.message}"`, { timeout: 7 }),
            );
            return;
        }
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

        let proverStart = new Date();

        yield combineMessages(
            info(`Prover <code>${pq_op.prover_run.worker}</code> is `
                + `proving block <code>${pq_op.prover_run.block_number}</code> `
                + `for <code>${Math.round((new Date() - proverStart) / 1000)}</code> seconds`),
            start_progress_bar({variant: 'half', duration: timeConstants.provingHalfLife}) 
        );

        while ( ! pq_op.verified) {
            yield info(`Prover <code>${pq_op.prover_run.worker}</code> is `
                + `proving block <code>${pq_op.prover_run.block_number}</code> `
                + `for <code>${Math.round((new Date() - proverStart) / 1000)}</code> seconds`);

            pq_op = await this.wallet.provider.getPriorityOpReceipt(pq_id);

            await sleep(1000);
        }
        
        yield combineMessages(
            info (`Priority op <code>${pq_id}</code> got proved!`, { countdown: 10 }),
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
