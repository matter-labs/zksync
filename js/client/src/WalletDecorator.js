import {Contract, utils} from 'ethers';
import { readableEther, sleep, isReadablyPrintable } from './utils';
import timeConstants from './timeConstants';
import { BlockExplorerClient } from './BlockExplorerClient';
import config from './env-config';
const zksync = require('zksync');
const ethers = require('ethers');
import franklin_abi from '../../../contracts/build/Franklin.json'
import { Emitter } from './Emitter';


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

function tokenInfoFromToken(token) {
    if (token === 'ETH') {
        return window.tokensList[0];
    }

    let info = window.tokensList.filter(t => t.address === token);

    return info.length ? info[0] : { symbol: 'NAI' };
}

function tokenInfoFromSymbol(symbol) {
    if (symbol === 'ETH') {
        return window.tokensList[0];
    }

    let info = window.tokensList.filter(t => t.symbol === symbol);

    return info.length ? info[0] : { symbol: 'NAI' };
}

function shortenedTxHash(tx_hash) {
    return `<code class="clickable copyable" data-clipboard-text="${tx_hash}">
                ${ tx_hash.substr(0, 10) }
            </code>`;
}

export class WalletDecorator {
    // #region everything
    constructor () {
        this.address = window.syncWallet.address();
        this.blockExplorerClient = new BlockExplorerClient(config.API_SERVER);
    }

    static async new(wallet) {
        let res = new WalletDecorator();
        res.ethAddress = await window.ethSigner.getAddress();
        return res;
    }

    async getDepositFeeReadable() {
        return readableEther(await this.getDepositFee());
    }

    async getDepositFee(token) {
        const gasPrice = await window.ethSigner.provider.getGasPrice();
        const multiplier = token == "ETH" ? 179000 : 214000;
        const redundancyCoef = 5;
        return gasPrice.mul(redundancyCoef * 2 * multiplier);
    }

    async updateState() {
        const onchainBalances = await Promise.all(
            window.tokensList.map(
                async tokenInfo => {
                    const token = tokenInfo.symbol === 'ETH' 
                        ? 'ETH' 
                        : tokenInfo.address;

                    return await window.syncWallet.getEthereumBalance(token);
                }
            )
        );
        
        this.ethState = {onchainBalances};
        this.syncState = await window.syncWallet.getAccountState();
    }

    tokenNameFromId(tokenId) {
        let token = window.tokensList[tokenId];
        if (token.symbol) {
            return token.symbol;
        } else {
            return `erc20_${tokenId}`;
        }
    }

    tokenFromName(tokenName) {
        let first = window.tokensList.filter(token => token.symbol == tokenName);
        if (first.length) return first[0];
        let tokenId = tokenName.slice('erc20_'.length);
        let second = window.tokensList.filter(token => {
            return token.id == tokenId;
        });
        return second[0];
    }

    async waitTxMine(hash) {
        let tx;
        do {
            tx = await window.ethProvider.getTransaction(hash);
        } while (tx.blockHash || await sleep(2000));
        return tx;
    }
    // #endregion

    // #region renderable
    async transactionsAsRenderableList(offset, limit) {
        const address = this.address;
        if (!address) {
            console.log(address);
            return [];
        }
        const transactions = await this.blockExplorerClient.getAccountTransactions(address, offset, limit);
        const res = transactions.map(async (tx, index) => {
            const elem_id      = `history_${index}`;
            const type         = tx.tx.type || '';
            const hash         = tx.hash;

            const receiver_address = type == 'Deposit'
                ? tx.tx.priority_op.to
                : tx.tx.to;

            const direction = receiver_address == address
                ? 'incoming' 
                : 'outcoming';

            // pub hash: Option<String>,
            // pub tx: Value,
            // pub success: Option<bool>,
            // pub fail_reason: Option<String>,
            // pub commited: bool,
            // pub verified: bool,

            const status
                = tx.verified            ? `<span style="color: green">(verified)</span>`
                : tx.success == null     ? `<span style="color: grey">(pending)</span>`
                : tx.success == true     ? `<span style="color: grey">(succeeded)</span>`
                : tx.commited            ? `<span style="color: grey">(committed)</span>`
                : tx.fail_reason != null ? `<span style="color: red">(failed)</span>`
                : `<span style="color: red">(Unknown status)</span>`;

            const row_status
                = tx.verified     ? `<span style="color: green">Verified</span>`
                : tx.commited     ? `<span style="color: grey">Committed</span>`
                : tx.fail_reason  ? `<span style="color: red">Failed with ${tx.fail_reason}</span>`
                : `<span style="color: red">(Unknown status)</span>`;

            // here is common data to all tx types
            const data = {
                elem_id,
                type, direction,
                status, row_status,
                hash,
            };

            switch (true) {
                case type == 'Deposit': {
                    const token = await this.tokenNameFromId(tx.tx.priority_op.token);
                    const amount = readableEther(tx.tx.priority_op.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'pq_id',       label: 'Priority op' },
                        ],
                        data: {
                            ...data,
                            from: tx.tx.priority_op.from,
                            to: tx.tx.priority_op.to,
                            pq_id: tx.pq_id,
                            token, amount,
                        },
                    };
                }
                case type == 'Transfer': {
                    const token = await this.tokenNameFromId(tx.tx.token);
                    const amount = readableEther(tx.tx.amount);
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'to',          label: 'To' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: {
                            ...data,
                            from: tx.tx.from,
                            to: tx.tx.to,
                            token, amount,
                        },
                    };
                }
                case type == 'Withdraw': {
                    const token = await this.tokenNameFromId(tx.tx.token);
                    const amount = readableEther(tx.tx.amount);   
                    return {
                        fields: [
                            { key: 'amount',      label: 'Amount' },
                            { key: 'row_status',  label: 'Status' },
                            { key: 'hash',        label: 'Tx hash' },
                        ],
                        data: {
                            ...data,
                            from: tx.tx.from,
                            to: tx.tx.to,
                            token, amount,
                        },
                    };
                }
            }
        });

        const txs = await Promise.all(res);
        return txs.filter(Boolean);
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
        const contract = new Contract(
            window.syncProvider.contractAddress.mainContract,
            franklin_abi.interface, 
            window.ethSigner
        );

        const balances = await Promise.all(
            window.tokensList
                .map(async token => {
                    const amount = await contract.balancesToWithdraw(
                        await window.ethSigner.getAddress(),
                        token.id
                    );
    
                    return { token, amount };
                }
            )
        );

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
        return this.ethState.onchainBalances
            .map((balance, tokenId) => ({
                tokenName: this.tokenNameFromId(tokenId),
                address: window.tokensList[tokenId].address,
                amount: balance.toString()
            }))
            .filter(tokenInfo => Number(tokenInfo.amount));
    }
    // contractBalancesAsRenderableList() {
    //     return this.ethState.contractBalances
    //         .map((balance, tokenId) => ({
    //             tokenName: this.tokenNameFromId(tokenId),
    //             address: window.tokensList[tokenId].address,
    //             amount: `${balance.toString()}`
    //         }))
    //         .filter(tokenInfo => Number(tokenInfo.amount));
    // }
    franklinBalancesAsRenderableListWithInfo() {
        if (this.syncState == undefined) return [];

        let res = {};
        let assign = key => ([token, balance]) => {
            let tokenInfo = tokenInfoFromSymbol(token);
            if (res[tokenInfo.id] === undefined) {
                res[tokenInfo.id] = {
                    tokenName: tokenInfo.symbol,
                    address: tokenInfo.token,
                };
            }
            res[tokenInfo.id][key] = balance;
        }
        Object.entries(this.syncState.committed.balances).forEach(assign('committedAmount'));
        Object.entries(this.syncState.verified.balances).forEach(assign('verifiedAmount'));
        return Object.values(res)
            .map(val => {
                val['committedAmount'] = val['committedAmount'] || utils.bigNumberify(0);
                val['verifiedAmount']  = val['verifiedAmount']  || utils.bigNumberify(0);
                val.verified           = val.verifiedAmount     == val.committedAmount;
                return val;
            })
            .filter(entry => Number(entry.committedAmount) || Number(entry.verifiedAmount));
    }
    franklinBalancesAsRenderableList() {
        return Object.entries(this.syncState.committed.balances)
            .map(([token, balance]) => {
                const tokenInfo = tokenInfoFromSymbol(token);
                return {
                    tokenInfo,
                    tokenName: tokenInfo.symbol,
                    amount: balance
                };
            })
            .filter(bal => Number(bal.amount))
            .sort((a, b) => a.tokenInfo.id - b.tokenInfo.id);
        }
    // #endregion

    // #region actions
    async * verboseTransfer(options) {
        const tokenInfo = tokenInfoFromSymbol(options.token);
        const token   = options.token === "ETH" ? "ETH" : tokenInfo.address;
        const amount  = utils.bigNumberify(options.amount);
        const fee     = utils.bigNumberify(options.fee);
        const address = options.address;

        try {
            if (!await window.syncWallet.isSigningKeySet()) {
                yield info(`Changing signing key...`);
                const setPk = await window.syncWallet.setSigningKey();
                await setPk.awaitReceipt();
            }

            yield info(`Sending transfer...`);

            const transferTransaction = await window.syncWallet.syncTransfer({
                to: address,
                token,
                amount, 
                fee
            });
    
            yield info(`Sent transfer to Matter server`);
    
            yield * this.verboseGetSyncOpStatus(transferTransaction);

            yield info(`Transfer succeeded!`);
            return;
        } catch (e) {
            console.log(JSON.stringify(e, null, 2));
            yield error(`Sending transfer failed with ${e.message}`);
            return;
        }
    }

    async * verboseWithdrawOffchain(options) {
        const tokenInfo = tokenInfoFromSymbol(options.token);
        const token   = options.token === "ETH" ? "ETH" : tokenInfo.address;
        const amount  = utils.bigNumberify(options.amount);
        const fee     = utils.bigNumberify(options.fee);

        try {
            if (!await window.syncWallet.isSigningKeySet()) {
                yield info(`Changing signing key...`);
                const setPk = await window.syncWallet.setSigningKey();
                await setPk.awaitReceipt();
            }

            yield info(`Sending withdraw...`);

            const withdrawTransaction = await window.syncWallet.withdrawFromSyncToEthereum({
                ethAddress: await window.ethSigner.getAddress(),
                token,
                amount,
                fee,
            });

            yield info(`Sent withdraw to Matter server`);

            yield * this.verboseGetSyncOpStatus(withdrawTransaction);

            yield info(`Withdraw succeeded!`);
        } catch (e) {
            console.log(JSON.stringify(e, null, 2));
            yield combineMessages(
                error(`Withdraw failed with ${e.message}`, { timeout: 7 }),
            );
            return;
        }
    }

    async * verboseWithdrawOnchain(options) {
        const token = options.token === "ETH" ? "ETH" : tokenInfoFromSymbol(options.token).address;
        const amount = utils.bigNumberify(options.amount);

        try {
            this.setPendingWithdrawStatus(`${token.id}`, 'loading');

            yield info(`Completing withdraw...`);
            const contract = new Contract(
                window.syncProvider.contractAddress.mainContract,
                franklin_abi.interface, 
                window.ethSigner
            );
            if (token == "ETH") {
                await contract.withdrawETH(amount);
            } else {
                await contract.withdrawERC20(token, amount, {gasLimit: bigNumberify("150000")});
            }

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

    async revertReason(tx_hash) {
        const tx = await window.ethProvider.getTransaction(tx_hash);
        const code = await window.ethProvider.call(tx, tx.blockNumber);

        if (code == '0x') {
            return '';
        } else {
            return code
                .substr(138)
                .match(/../g)
                .map(h => parseInt(h, 16))
                .map(String.fromCharCode)
                .join('')
                .split('')
                .filter(c => /\w/.test(c))
                .join('');
        }
    }

    async * verboseDeposit(options) {
        const token = options.token === "ETH" ? "ETH" : tokenInfoFromSymbol(options.token).address;
        const amount = utils.bigNumberify(options.amount);
        try {
            yield info(`Sending deposit...`);

            const maxFeeInETHToken = await this.getDepositFee(token);
            const deposit = await window.syncWallet.depositToSyncFromEthereum({
                depositTo: window.syncWallet.address(),
                maxFeeInETHToken,
                token,
                amount,
                approveDepositAmountForERC20: true,
            });

            await deposit.awaitEthereumTxCommit();

            const txHashHtml = shortenedTxHash(deposit.ethTx.hash);
            yield info(`Deposit ${txHashHtml} sent to Mainchain...`);

            
            yield * this.verboseGetRevertReason(deposit.ethTx.hash);
            
            yield * this.verboseGetSyncPriorityOpStatus(deposit);

            yield info(`Deposit succeeded!`);
        } catch (e) {
            yield combineMessages(
                info(`Onchain deposit failed with "${e.message}"`, { countdown: 7 }),
            );
            await sleep(5000);
            return;
        }
    }

    async * verboseGetSyncOpStatus(syncOp) {
        this.emit("receiptCommittedOrVerified");

        const txHashHtml = shortenedTxHash(syncOp.txHash);
    
        const receipt = await syncOp.awaitReceipt();
        this.emit("receiptCommittedOrVerified"); 
        console.log('awaitReceipt');

        if (receipt.failReason) {
            yield error(`Transaction ${txHashHtml} with <code>${receipt.failReason}</code>`, { countdown: 10 });
            return;
        }

        yield combineMessages(
            info(`Transaction ${txHashHtml} got included in block <code>${receipt.block.blockNumber}</code>, waiting for prover...`),
            start_progress_bar({variant: 'half', duration: timeConstants.waitingForProverHalfLife})
        );

        let verified = false;
        syncOp.awaitVerifyReceipt()
            .then(verifyReceipt => {
                this.emit("receiptCommittedOrVerified");
                console.log('verifyReceipt');
                verified = true;
            });

        const proverStart = new Date();

        const sendProvingFrame = () => info(
            `Block <code>${receipt.block.blockNumber}</code> is being proved `
            + `for <code>${Math.round((new Date() - proverStart) / 1000)}</code> seconds`
        );

        yield combineMessages(
            sendProvingFrame(),
            start_progress_bar({variant: 'half', duration: timeConstants.provingHalfLife}) 
        );

        while (verified == false) {
            yield sendProvingFrame();
            await sleep(1000);
        }

        yield combineMessages(
            info(`Transaction ${txHashHtml} got proved!`, { countdown: 10 }),
            stop_progress_bar()
        );
    }

    async * verboseGetSyncPriorityOpStatus(syncOp) {
        this.emit("receiptCommittedOrVerified");

        let txHashHtml = shortenedTxHash(syncOp.ethTx.hash);

        await syncOp.awaitEthereumTxCommit();

        const receipt = await syncOp.awaitReceipt();
        this.emit("receiptCommittedOrVerified");
        console.log('awaitReceipt')
        
        yield combineMessages(
            info(`Transaction ${txHashHtml} got included in block <code>${receipt.block.blockNumber}</code>, waiting for prover...`),
            start_progress_bar({variant: 'half', duration: timeConstants.waitingForProverHalfLife})
        );

        let verified = false;
        syncOp.awaitVerifyReceipt()
            .then(verifyReceipt => {
                this.emit("receiptCommittedOrVerified");
                console.log('verifyReceipt');
                verified = true;
            });

        const proverStart = new Date();

        const sendProvingFrame = () => info(
            `Block <code>${receipt.block.blockNumber}</code> is being proved `
            + `for <code>${Math.round((new Date() - proverStart) / 1000)}</code> seconds`
        );

        yield combineMessages(
            sendProvingFrame(),
            start_progress_bar({variant: 'half', duration: timeConstants.provingHalfLife}) 
        );

        while (verified == false) {
            yield sendProvingFrame();
            await sleep(1000);
        }

        yield combineMessages(
            info(`Transaction ${txHashHtml} got proved!`, { countdown: 10 }),
            stop_progress_bar()
        );
        return;
    }

    async * verboseGetRevertReason(txHash) {
        const txHashHtml = shortenedTxHash(txHash);
        yield combineMessages(
            info(`Waiting for transaction ${txHashHtml} to mine...`),
            start_progress_bar({variant: 'half', duration: timeConstants.ethereumMiningHalfLife})
        );

        let receipt;
        while (true) {
            receipt = await window.ethProvider.getTransactionReceipt(txHash);
            
            if (receipt) break;
            await sleep(timeConstants.ethereumReceiptRetry);
        }

        if (receipt.status) {
            yield combineMessages(
                info(`Transaction ${txHashHtml} succeeded.`),
                stop_progress_bar()
            );
        } else {
            const tx = await window.ethProvider.getTransaction(txHash);
            const code = await window.ethProvider.call(tx, tx.blockNumber);

            if (code == '0x') {
                yield error(`Transaction ${txHashHtml} failed with empty revert reason.`);
            } else {
                const reason = code
                    .substr(138)
                    .match(/../g)
                    .map(h => parseInt(h, 16))
                    .map(String.fromCharCode)
                    .join('')
                    .split('')
                    .filter(c => /\w/.test(c))
                    .join('');
                yield error(`Transaction ${txHashHtml} failed with <code>${reason}<code>.`);
            }
        }
    }

    // #endregion
}

Emitter(WalletDecorator.prototype);
