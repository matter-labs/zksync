import { BigNumberish, BigNumber } from 'ethers/utils';
import { FranklinProvider, Wallet, Address } from 'franklin_lib'

export class WalletDecorator {
    constructor (wallet) {
        this.wallet = wallet;
    }

    async updateState() {
        await this.wallet.updateState();
    }

    tokenNameFromId(tokenId) {
        console.log(tokenId);
        let token = this.wallet.supportedTokens[tokenId];
        let res = token.symbol;
        if (res) return res;
        return `erc20${tokenId}`;
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
}
