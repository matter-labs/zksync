import { AbstractOperation } from './AbstractOperation'
import { Wallet, Token } from '../../src/wallet';
import { LocalWallet } from './LocalWallet';
import { BigNumberish, BigNumber } from 'ethers/utils';

interface DepositOperationKwargs {
    wallet: LocalWallet,
    token: Token,
    amount: BigNumber,
    fee: BigNumber
}

export class DepositOperation extends AbstractOperation {
    constructor(protected kwargs: DepositOperationKwargs) {
        super(kwargs.wallet);
    }

    public async action() {
        this.logStart(`trying deposit(${this.kwargs.token.id} | ${this.kwargs.amount.toString()})`);
        await this.kwargs.wallet.deposit(this.kwargs.token, this.kwargs.amount, this.kwargs.fee);
        await this.kwargs.wallet.franklinWallet.updateState();
    }
}
