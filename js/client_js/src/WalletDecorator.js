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
    
    async depositOnchain(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let res = await this.wallet.depositOnchain(token, amount);
        console.log(res);
    }

    async depositOffchain(kwargs) {
        let token = this.tokenFromName(kwargs.token);
        let amount = bigNumberify(kwargs.amount);
        let fee = bigNumberify(0);

        let res = await this.wallet.depositOffchain(token, amount, fee);
        console.log(res);
        if (res.err) {
            throw new Error(res.err);
        }
        let receipt = await this.wallet.txReceipt(res.hash);
        console.log(receipt);
        if (receipt.fail_reason) {
            throw new Error(receipt.fail_reason);
        }
        return 0;
    }
}
