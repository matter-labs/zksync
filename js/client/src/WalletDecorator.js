import { BigNumberish, BigNumber, bigNumberify, Interface } from 'ethers/utils';
import { Contract } from 'ethers';
import { FranklinProvider, Wallet, Address } from 'franklin_lib';
import { readableEther, sleep, isReadablyPrintable } from './utils';
import timeConstants from './timeConstants';
import IERC20Conract from '../../franklin_lib/abi/IERC20.json';
import config from './env-config';

import priority_queue_abi from '../../../contracts/build/PriorityQueue.json'

const NUMERIC_LIMITS_UINT_256 = '115792089237316195423570985008687907853269984665640564039457584007913129639935';

// #region communication with AlertWithProgressBar
function combineMessages(...args) {
    return Object.assign({}, ...args);
}

function info(msg, options) {
    let displayMessage = {
        message: msg,
        error: false,
        variant: 'success',
        countdown: 0, // infinity
    };
    
    // update default displayMessage with values from options dict.
    Object.assign(displayMessage, options);

    return { displayMessage };
}

function error(msg, options) {
    let displayMessage = {
        message: msg,
        error: true,
        variant: 'danger',
        countdown: 0, // infinity
    };
    
    // update default displayMessage with values from options dict.
    Object.assign(displayMessage, options);

    return { displayMessage };
};

function start_progress_bar(options) {
    let startProgressBar = {
        variant: 'half',
        duration: timeConstants.waitingForProverHalfLife,
    };

    // update default startProgressBar with values from options dict.
    Object.assign(startProgressBar, options);

    return { startProgressBar };
}

function stop_progress_bar() {
    let stopProgressBar = {};

    return { stopProgressBar };
}
// #endregion

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

    async waitTxMine(hash) {
        let tx;
        do {
            tx = await this.wallet.ethWallet.provider.getTransaction(hash);
        } while (tx.blockHash || await sleep(2000));
        return tx;
    }
    // #endregion

    // #region renderable
    async transactionsAsRenderableList(offset, limit) {
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
                : tx.fail_reason  ? `<span style="color: red">Failed with ${tx.fail_reason}</span>`
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
        let allowances = await this.wallet.getAllowancesForAllTokens();
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
    
    setPendingWithdrawStatus(withdrawTokenId, status) {
        let withdrawsStatusesDict = JSON.parse(localStorage.getItem('withdrawsStatusesDict') || "{}");
        withdrawsStatusesDict[withdrawTokenId] = status;
        localStorage.setItem('withdrawsStatusesDict', JSON.stringify(withdrawsStatusesDict));
    }
    removePendingWithdraw(withdrawTokenId) {
        let withdrawsStatusesDict = JSON.parse(localStorage.getItem('withdrawsStatusesDict') || "{}");
        delete withdrawsStatusesDict[withdrawTokenId];
        localStorage.setItem('withdrawsStatusesDict', JSON.stringify(withdrawsStatusesDict));
    }
    getWithdrawsStatusesDict(withdrawTokenId) {
        return JSON.parse(localStorage.getItem('withdrawsStatusesDict') || "{}")[withdrawTokenId];
    }
    async pendingWithdrawsAsRenderableList() {
        let balances = await this.wallet.getBalancesToWithdraw();
        return balances
            .map(a => ({
                token: a.token,
                amount: a.amount.toString()
            }))
            .filter(bal => bal.token.symbol == 'ETH' ? bal.amount.length > 15 : bal.amount != '0')
            .map((op, i) => {
                op.operation = 'Withdraw';
                op.uniq_id = `${op.token.id}`,
                op.elem_id = `pendingWithdraw_${i}`;
                op.amountRenderable = readableEther(op.amount);
                op.status = this.getWithdrawsStatusesDict(op.uniq_id);
                return op;
            });
    }
    async pendingOperationsAsRenderableList() {
        return await this.pendingWithdrawsAsRenderableList();
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
        let assign = key => ([tokenId, balance]) => {
            if (res[tokenId] === undefined) {
                res[tokenId] = {
                    tokenName: this.tokenNameFromId(tokenId),
                    address: this.wallet.supportedTokens[tokenId].address,
                };
            }
            res[tokenId][key] = balance;
        };
        Object.entries(this.wallet.franklinState.commited.balances).forEach(assign('committedAmount'));
        Object.entries(this.wallet.franklinState.verified.balances).forEach(assign('verifiedAmount'));
        return Object.values(res)
            .map(val => {
                val['committedAmount'] = val['committedAmount'] || bigNumberify(0);
                val['verifiedAmount']  = val['verifiedAmount']  || bigNumberify(0);
                val.verified           = val.verifiedAmount     == val.committedAmount;
                return val;
            })
            .filter(entry => Number(entry.committedAmount) || Number(entry.verifiedAmount));
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
    async * verboseTransfer(options) {
        let token = this.tokenFromName(options.token);
        let amount = bigNumberify(options.amount);
        let fee = bigNumberify(options.fee);
        let address = options.address;

        try {
            let fra_tx = await this.wallet.transfer(address, token, amount, fee);
    
            if (fra_tx.err) {
                yield error(`Transfer failed with ${fra_tx.err}`);
                return;
            }

            yield info(`Sent transfer to Matter server`);
    
            yield * this.verboseGetFranklinOpStatus(fra_tx.hash);
            return;
        } catch (e) {
            yield error(`Sending transfer failed with ${e.message}`);
            return;
        }
    }

    async * verboseWithdrawOffchain(options) {
        let token = this.tokenFromName(options.token);
        let amount = bigNumberify(options.amount);
        let fee = bigNumberify(options.fee);

        try {
            yield info(`Sending withdraw...`);

            let fra_tx = await this.wallet.widthdrawOffchain(token, amount, fee);
            this.removePendingWithdraw(`${token.id}`);
            
            if (fra_tx.err) {
                yield error(`Offchain withdraw failed with ${fra_tx.err}`);
                return;
            }

            yield * this.verboseGetFranklinOpStatus(fra_tx.hash);

            yield info(`Offchain withdraw succeeded!`);
        } catch (e) {
            yield combineMessages(
                error('Withdraw failed with ', e.message, { timeout: 7 }),
            );
            return;
        }
    }

    async * verboseWithdrawOnchain(options) {
        let token = options.token
        let amount = bigNumberify(options.amount);

        try {
            this.setPendingWithdrawStatus(`${token.id}`, 'loading');

            yield info(`Completing withdraw...`);
            let eth_tx = await this.wallet.widthdrawOnchain(token, amount);
            
            this.setPendingWithdrawStatus(`${token.id}`, 'hidden');
            
            await eth_tx.wait(2);
            yield * this.verboseGetRevertReason(eth_tx.hash);
    
            yield info(`Withdraw succeeded!`);
            this.removePendingWithdraw(`${token.id}`);
        } catch (e) {
            yield combineMessages(
                error('Withdraw failed with ', e.message, { timeout: 7 }),
            );
            return;
        }
    }

    async * verboseDeposit(options) {
        let token = this.tokenFromName(options.token);
        let amount = bigNumberify(options.amount);

        try {
            yield info(`Sending deposit...`);
            
            let eth_tx;
            if (token.symbol == 'ETH') {
                eth_tx = await this.wallet.depositETH(amount);
            } else {
                let erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.wallet.ethWallet);
                let allowance = await erc20DeployedToken.allowance(this.ethAddress, config.CONTRACT_ADDR);
                if (allowance.toString().length != NUMERIC_LIMITS_UINT_256.length) {
                    let nonce = await this.wallet.ethWallet.getTransactionCount();
                    this.wallet.approveERC20(token, NUMERIC_LIMITS_UINT_256, { nonce });
                    eth_tx = await this.wallet.depositApprovedERC20(token, amount, { nonce: nonce + 1});
                } else {
                    eth_tx = await this.wallet.depositApprovedERC20(token, amount);
                }
            }

            const tx_hash_html = shortenedTxHash(eth_tx.hash);
            yield info(`Deposit ${tx_hash_html} sent to Mainchain...`);

            yield * await this.verboseGetRevertReason(eth_tx.hash);
            
            yield * this.verboseGetPriorityOpStatus(eth_tx.hash);
        } catch (e) {
            yield combineMessages(
                error(`Onchain deposit failed with "${e.message}"`, { timeout: 7 }),
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
