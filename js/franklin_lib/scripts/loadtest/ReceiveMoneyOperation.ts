import { AbstractOperation } from './AbstractOperation'
import { Wallet, Token } from '../../src/wallet';
import { LocalWallet } from './LocalWallet';
import { bigNumberify, BigNumberish, BigNumber } from 'ethers/utils';
import { Contract, ethers } from 'ethers';
const IERC20Conract = require('openzeppelin-solidity/build/contracts/ERC20Mintable.json');

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

interface ReceiveMoneyOperationKwargs {
    wallet: LocalWallet,
    token: Token,
    amount: BigNumber
}

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)
const richWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC).connect(provider);
let nonce = null;

// Wallet.fromEthWallet(richWallet)
//     .then(wallet => wallet.getOnchainBalances())
//     .then(console.log);

export class ReceiveMoneyOperation extends AbstractOperation {
    constructor(protected kwargs: ReceiveMoneyOperationKwargs) {
        super(kwargs.wallet);
    }

    public async action(): Promise<void> {
        this.logStart(`trying receiving money (${this.kwargs.token.id} | ${this.kwargs.amount.toString()})`);
        
        if (nonce == null) {
            nonce = await richWallet.getTransactionCount("pending");
        }

        await this.kwargs.wallet.receiveMoney(richWallet, this.kwargs.token, this.kwargs.amount, nonce++);

        await this.kwargs.wallet.updateState();
    }
}
