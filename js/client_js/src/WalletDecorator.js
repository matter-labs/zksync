import { BigNumberish, BigNumber, bigNumberify } from 'ethers/utils';
import { FranklinProvider, Wallet, Address } from 'franklin_lib'

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
        return this.wallet.franklinState.tx_history.map((tx, index) => {
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
                amount: balance.toString()
            }))
            .filter(tokenInfo => tokenInfo.amount);
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
        
        let res = await this.wallet.transfer(kwargs.address, token, amount, fee);

        if (res.err) throw new Error(res.err);
        let receipt = await this.wallet.txReceipt(res.hash);
        if (receipt.fail_reason) throw new Error(receipt.fail_reason);
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

        try {
            let _ = await this.depositOffchain(kwargs);
            yield {
                error: null,
                message: `Offchain deposit succeeded.`
            }
            return;
        } catch (e) {
            yield {
                error: e.message,
                message: `Offchain deposit failed with "${e.message}"`,
            };
            return;
        }
    }
}
