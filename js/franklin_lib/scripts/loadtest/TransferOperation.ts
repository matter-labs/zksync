import { AbstractOperation } from './AbstractOperation'
import { Wallet, Token } from '../../src/wallet';
import { LocalWallet } from './LocalWallet';
import { BigNumberish, BigNumber } from 'ethers/utils';

interface TransferOperationKwargs {
    wallet1: LocalWallet,
    wallet2: LocalWallet,
    token: Token,
    amount: BigNumber,
    fee: BigNumber
}

export class TransferOperation extends AbstractOperation {
    constructor(protected kwargs: TransferOperationKwargs) {
        super(kwargs.wallet1);
    }

    public async action(): Promise<void> {
        this.logStart(`trying transfer (${this.kwargs.token.id} | ${this.kwargs.amount.toString()} | ${this.kwargs.fee.toString()})`);
        await this.kwargs.wallet1.sendTransaction(this.kwargs.wallet2, this.kwargs.token, this.kwargs.amount, this.kwargs.fee);
    }
}
