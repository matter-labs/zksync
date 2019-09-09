import { AbstractOperation } from './AbstractOperation'
import { Wallet, Token } from '../../src/wallet';
import { LocalWallet } from './LocalWallet';
import { bigNumberify, BigNumberish, BigNumber } from 'ethers/utils';
import { Contract, ethers } from 'ethers';
const IERC20Conract = require('openzeppelin-solidity/build/contracts/ERC20Mintable.json');

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

interface ReceiveMoneyOperationKwargs {
    wallet: LocalWallet,
    token: Token,
    amount: BigNumber
}

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)

class RichEthWallet {
    source: ethers.Wallet;
    franklinWallet: Wallet;
    sourceNonce: number;

    private static instance: RichEthWallet = null;
    public static async getInstance(): Promise<RichEthWallet> {
        if (RichEthWallet.instance === null) {
            RichEthWallet.instance = undefined;
            RichEthWallet.instance = await RichEthWallet.new();
        }

        while (!RichEthWallet.instance) {
            await sleep(1399);
        }
        
        return RichEthWallet.instance;
    }

    private static async new(): Promise<RichEthWallet> {
        let wallet = new RichEthWallet();
        await wallet.prepare();

        wallet.franklinWallet = await Wallet.fromEthWallet(wallet.source);
        await wallet.franklinWallet.updateState();
        let amountsString = wallet.franklinWallet.ethState.onchainBalances
            .map((val, idx) => `token ${idx}: ${val.toString()}`);

        console.log(`RichEthWallet has amounts ${amountsString}`)
        
        return wallet;
    }

    private constructor() {
        this.source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    }

    private async prepare() {
        this.sourceNonce = await this.source.getTransactionCount("pending")
    }

    async transferOnchain(toEthAddress: string, tokenAddress: string, amount: BigNumberish, nonce: number) {
        const contract = new Contract(tokenAddress, IERC20Conract.abi, this.franklinWallet.ethWallet);
        let tx = await contract.transfer(toEthAddress, bigNumberify(amount), {nonce: nonce, gasLimit: bigNumberify("150000")});
        // console.log("TX: ", tx);
        await tx.wait(2)
        let receipt = await this.franklinWallet.ethWallet.provider.getTransactionReceipt(tx.hash);
        // console.log('transferOnchain receipt:', receipt);
        return tx;
    }

    async sendSome(wallet: LocalWallet, token: Token, amount: BigNumberish) {
        let to = wallet.franklinWallet.ethWallet;
        let txAddr = await to.getAddress();
        let txAmount = amount;
        let txNonce = this.sourceNonce
        
        ++this.sourceNonce;

        if (token.id == 0) {
            let promiseToSend = this.source.sendTransaction({
                to:     txAddr,
                value:  txAmount,
                nonce:  txNonce,
            });
            
            let mining = await promiseToSend;
            let mined = await mining.wait();
            // console.log(`${txAddr} onchain ${await to.provider.getBalance(txAddr)}`);
    
            wallet.addToComputedOnchainBalance(token, txAmount);
            // console.log(wallet.computedOnchainBalances);
    
            return mined;
        } else {
            let toAddress = await wallet.franklinWallet.ethWallet.getAddress();
            let res = await this.transferOnchain(toAddress, token.address, amount, txNonce);
            // console.log('res: ', res);
            wallet.addToComputedOnchainBalance(token, txAmount);
            return res;
        }
    }
}
export class ReceiveMoneyOperation extends AbstractOperation {
    constructor(protected kwargs: ReceiveMoneyOperationKwargs) {
        super(kwargs.wallet);
    }

    public async action(): Promise<void> {
        this.logStart(`trying receiving money (${this.kwargs.token.id} | ${this.kwargs.amount.toString()})`);
        const richWallet = await RichEthWallet.getInstance();
        await richWallet.sendSome(this.kwargs.wallet, this.kwargs.token, this.kwargs.amount)
    }
}
