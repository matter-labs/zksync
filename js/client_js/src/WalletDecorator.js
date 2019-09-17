import { BigNumberish, BigNumber, bigNumberify } from 'ethers/utils';
import { FranklinProvider, Wallet, Address } from 'franklin_lib'

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export class WalletDecorator {
    constructor (wallet) {
        this.wallet = wallet;
    }

    async updateState() {
        await this.wallet.updateState();
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
            console.log(tokenId);
            console.log(token);
            return token.id == tokenId;
        });
        return second[0];
    }

    // #region renderable
    transactionsAsNeeded() {
        return (this.wallet.franklinState.tx_history || []).map((tx, index) => {
            let elem_id      = `history_${index}`;
            let tx_hash      = null;
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
            return {
                type, to, amount, success, fail_reason, 
                is_committed, is_verified, elem_id
            };
        });
    }

    onchainBalancesAsRenderableList() {
        return this.wallet.ethState.onchainBalances
            .map((balance, tokenId) => ({
                tokenName: this.tokenNameFromId(tokenId),
                amount: balance.toString()
            }))
            .filter(tokenInfo => tokenInfo.amount);
    }
    contractBalancesAsRenderableList() {
        return this.wallet.ethState.contractBalances
            .map((balance, tokenId) => ({
                tokenName: this.tokenNameFromId(tokenId),
                amount: `${balance.toString()}, blocks left ${this.wallet.ethState.lockedBlocksLeft[tokenId]}`
            }))
            .filter(tokenInfo => true || tokenInfo.amount);
    }
    franklinBalancesAsRenderableListWithInfo() {
        let res = {};
        let assign = key => entry => {
            let [tokenId, balance] = entry;
            if (res[tokenId] === undefined) {
                res[tokenId] = {
                    tokenName: this.tokenNameFromId(tokenId),
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
        });
    }
    franklinBalancesAsRenderableList() {
        return Object.entries(this.wallet.franklinState.commited.balances)
            .map(entry => {
                let [tokenId, balance] = entry;
                return {
                    tokenName: this.tokenNameFromId(tokenId),
                    amount: balance
                };
            });
    }
    // #endregion
    
    async transfer(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);
        
        console.log('walletDecorator.transfer', kwargs)

        let res = await this.wallet.transfer(kwargs.address, token, amount, fee);

        console.log(res);

        if (res.err) throw new Error(res.err);
        let receipt = await this.wallet.txReceipt(res.hash);
        if (receipt.fail_reason) throw new Error(receipt.fail_reason);
    }

    async * verboseTransfer(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);
        
        try {
            var res = await this.wallet.transfer(kwargs.address, token, amount, fee);
        } catch (e) {
            yield {
                error: e.message,
                message: `Transfer failed with ${e.message}`
            };
            return;
        }

        if (res.err === null) {
            yield {
                error: null,
                message: `Sent transfer to Matters server`
            };
        } else {
            yield {
                error: res.err,
                message: `Transfer failed with ${res.err}`
            };
            return;
        }

        let receipt = await this.wallet.txReceipt(res.hash);

        if (receipt.fail_reason) {
            yield {
                error: receipt.fail_reason,
                message: `Transaction failed with ${receipt.fail_reason}`
            };
            return;
        } else {
            yield {
                error: null,
                message: `Tx ${res.hash} got included in block ${receipt.block_number}, waiting for prover`,
            };
        }

        while ( ! receipt.prover_run) {
            receipt = await this.wallet.txReceipt(res.hash)
            await sleep(1000);
        }

        yield {
            error: null,
            message: `Prover ${receipt.prover_run.worker} started `
                + `proving block ${receipt.prover_run.block_number} `
                + `at ${receipt.prover_run.created_at}`
        };

        while ( ! receipt.verified) {
            receipt = await this.wallet.txReceipt(res.hash)
            await sleep(1000);
        }

        yield {
            error: null,
            message: `Transfer ${res.hash} got proved!`
        };   
    }

    async * verboseWithdraw(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);

        let res = await this.wallet.widthdrawOffchain(token, amount, fee);

        if (res.err) {
            yield {
                error: res.err,
                message: `Offchain withdraw failed with ${res.err}`,
            }; 
            return;
        } else {
            yield {
                error: null,
                message: `Sent withdraw tx to Franklin server`,
            };
        }
    
        let receipt = await this.wallet.txReceipt(res.hash);

        if (receipt.fail_reason) {
            yield {
                error: receipt.fail_reason,
                message: `Transaction failed with ${receipt.fail_reason}`
            };
            return;
        } else {
            yield {
                error: null,
                message: `Tx ${res.hash} got included in block ${receipt.block_number}, waiting for prover`,
            };
        }

        while ( ! receipt.prover_run) {
            receipt = await this.wallet.txReceipt(res.hash)
            await sleep(1000);
        }

        yield {
            error: null,
            message: `Prover ${receipt.prover_run.worker} started `
                + `proving block ${receipt.prover_run.block_number} `
                + `at ${receipt.prover_run.created_at}`
        };

        while ( ! receipt.verified) {
            receipt = await this.wallet.txReceipt(res.hash)
            await sleep(1000);
        }

        yield {
            error: null,
            message: `Tx ${res.hash} got proved! Starting onchain deposit after sleeping lol`
        };

        await sleep(5000);

        try {
            var hash = await this.wallet.widthdrawOnchain(token, amount);
        } catch (e) {
            yield {
                error: e.message,
                message: `Withdraw onchain failed with ${e.message}`
            };
            return;
        }

        yield {
            err: null,
            message: `Withdraw succeeded!`
        };
    }

    async depositOnchain(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let tx_hash = await this.wallet.depositOnchain(token, amount);
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

        let receipt = await this.wallet.txReceipt(res.hash);

        if (receipt.fail_reason) {
            throw new Error(receipt.fail_reason);
        }
        return 0;
    }

    async * verboseDepositOffchain(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(kwargs.fee);

        let res = await this.wallet.depositOffchain(token, amount, fee);

        if (res.err) {
            yield {
                error: res.err,
                message: `Onchain Deposit Failed with ${res.err}`,
            }; 
            return;
        } else {
            yield {
                error: null,
                message: `Sent tx to Franklin server`,
            };
        }

        let receipt = await this.wallet.txReceipt(res.hash);

        if (receipt.fail_reason) {
            yield {
                error: receipt.fail_reason,
                message: `Transaction failed with ${receipt.fail_reason}`
            };
            return;
        } else {
            yield {
                error: null,
                message: `Tx ${res.hash} got included in block ${receipt.block_number}, waiting for prover`,
            };
        }

        while ( ! receipt.prover_run) {
            receipt = await this.wallet.txReceipt(res.hash)
            await sleep(1000);
        }

        yield {
            error: null,
            message: `Prover ${receipt.prover_run.worker} started `
                + `proving block ${receipt.prover_run.block_number} `
                + `at ${receipt.prover_run.created_at}`
        };

        while ( ! receipt.verified) {
            receipt = await this.wallet.txReceipt(res.hash)
            await sleep(1000);
        }

        yield {
            error: null,
            message: `Tx ${res.hash} got proved!`
        }
    }

    async * verboseDeposit(kwargs) {
        yield {
            error: null,
            message: `Sending deposit...`
        }

        try {
            let tx_hash = await this.depositOnchain(kwargs);
            yield {
                error: null,
                tx_hash,
                message: `Onchain deposit succeeded, waiting for offchain...`,
            };
        } catch (e) {
            yield {
                error: e.message,
                message: `Onchain deposit failed with "${e.message}"`,
            };
            return;
        }

        yield * this.verboseDepositOffchain(kwargs);
    }
}
