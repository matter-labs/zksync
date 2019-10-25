import { AbstractOperation } from './AbstractOperation'
import { Wallet, Token } from '../../src/wallet';
import { LocalWallet } from './LocalWallet';
import { BigNumberish, BigNumber, parseEther, formatEther } from 'ethers/utils';

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
        this.logStart(`deposit(${this.kwargs.token.id} | ${formatEther(this.kwargs.amount)} | ${formatEther(this.kwargs.fee)})`);
        let handle = await this.kwargs.wallet.deposit(this.kwargs.token, this.kwargs.amount, this.kwargs.fee);
        // this.log(`Deposit tx hash is ${handle.ethTx.hash}`);
        await handle.waitCommit();
    }
}
