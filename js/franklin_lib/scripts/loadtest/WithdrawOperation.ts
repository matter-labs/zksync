import { AbstractOperation } from './AbstractOperation'
import { Wallet, Token } from '../../src/wallet';
import { LocalWallet } from './LocalWallet';
import { BigNumberish, BigNumber } from 'ethers/utils';

interface WithdrawOperationKwargs {
    wallet: LocalWallet,
    token: Token,
    amount: BigNumber,
    fee: BigNumber
}

export class WithdrawOperation extends AbstractOperation {
    constructor(protected kwargs: WithdrawOperationKwargs) {
        super(kwargs.wallet);
    }

    public async action() {
        this.logStart(`trying withdraw(${this.kwargs.token.id} | ${this.kwargs.amount.toString()} | ${this.kwargs.fee.toString()})`);
        await this.kwargs.wallet.withdraw(this.kwargs.token, this.kwargs.amount, this.kwargs.fee);
        await this.kwargs.wallet.franklinWallet.updateState();
    }
}
