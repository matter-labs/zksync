import { BigNumberish, BigNumber, bigNumberify } from 'ethers/utils';
import { FranklinProvider, Wallet, Address } from 'franklin_lib';
import { readableEther } from './utils';

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

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
        this.tx_history = await this.wallet.provider.getTransactionsHistory(this.address.substr(2));
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
        let res = (this.tx_history).map(async (tx, index) => {
            let elem_id      = `history_${index}`;
            let hash         = tx.tx_hash;
            let success      = tx.success     || '';
            let nonce        = tx.tx.nonce    || '';
            let from         = null;
            let type         = tx.tx.type     || '';
            let to           = tx.tx.to       || '';
            let token        = tx.tx.token    || '';
            let amount       = tx.tx.amount   || '';
            let fee          = tx.tx.fee      || '';
            let fail_reason  = tx.fail_reason || '';
            let is_committed = tx.committed   || '';
            let is_verified  = tx.verified    || '';

            let status = (() => {
                if (is_verified) return `<span style="color: green">(verified)</span>`;
                if (is_committed) return `<span style="color: grey">(committed)</span>`;
                if (success) return `<span style="color: grey">(succeeded)</span>`;
                if (fail_reason) return `<span style="color: red">(failed)</span>`;
                return `<span style="color: red">(WTF)</span>`;
            })();

            let status_tooltip = await (async () => {
                if ( ! is_committed) {
                    return 'Nothing';
                }
                
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
                // on hover, show tooltip with 
                //      "waiting for prover..."
                //      "prover <code>default</code> started proving at <code>created_at</code>"
                
                if (receipt == null || receipt.prover_run == null) {
                    return 'Waiting for prover..';
                }

                let prover_name = receipt.prover_run.worker;
                let started_time = receipt.prover_run.created;
                return `Is being proved since ${started_time}`;
            })();

            let row_status = await (async () => {
                if (is_verified) return `<span style="color: green">Verified</span>`;
                if (is_committed) return `<span style="color: grey">Committed</span>`;
                if (success) return `<span style="color: grey">Succeeded</span>`;
                if (fail_reason) return `<span style="color: red">Failed with ${fail_reason}</span>`;
            })();

            const directions = {
                in: `<span style="color: green">(in)</span>`,
                out: `<span style="color: red">(out)</span>`,
            };

            // TODO: add incoming transactions
            let direction = type == 'Deposit' ? 'incoming' : 'outcoming';

            return {
                type, to, amount, success, fail_reason, 
                is_committed, is_verified, elem_id,
                hash, status, status_tooltip, 
                row_status, direction,
            };
        });

        return Promise.all(res);
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
        } catch (e) {
            yield error('Withdraw failed with ', e.message);
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
            return;
        }

        try {
            yield * await this.verboseGetRevertReason(tx_hash);
        } catch (e) {
            yield error(`Onchain deposit failed with "${e.message}"`);
            return;
        }
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
