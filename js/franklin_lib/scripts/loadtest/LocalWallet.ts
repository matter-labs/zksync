import { AbstractOperation } from './AbstractOperation'
import { bigNumberify, BigNumber, BigNumberish } from "ethers/utils";
import { ethers } from 'ethers';
import { Wallet, Token } from '../../src/wallet';

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

export class LocalWallet {
    public resetNonce(): void {
        this.franklinWallet.nonce = null;
    }

    public franklinWallet: Wallet;
    public actions: AbstractOperation[] = [];
    public addAction(op: AbstractOperation): void {
        this.actions.push(op);
    }
    
    // #region computedBalances

    computedOnchainBalances = {};
    computedLockedBalances = {};
    computedFranklinBalances = {};
    static getComputedBalance(dict, token): BigNumber {
        if (dict[token.id] === undefined) {
            dict[token.id] = bigNumberify(0);
        }
        return dict[token.id];
    }
    getComputedOnchainBalance(token: Token): BigNumber {
        return LocalWallet.getComputedBalance(this.computedOnchainBalances, token);
    }
    getComputedLockedBalance(token: Token): BigNumber {
        return LocalWallet.getComputedBalance(this.computedLockedBalances, token);
    }
    getComputedFranklinBalance(token: Token): BigNumber {
        return LocalWallet.getComputedBalance(this.computedFranklinBalances, token);
    }

    addToComputedBalance(dict, token, amount: BigNumberish) {
        dict[token.id] = LocalWallet.getComputedBalance(dict, token).add(amount);
    }
    addToComputedOnchainBalance(token, amount: BigNumberish) {
        this.addToComputedBalance(this.computedOnchainBalances, token, amount);
    }
    addToComputedLockedBalance(token, amount: BigNumberish) {
        this.addToComputedBalance(this.computedLockedBalances, token, amount);
    }
    addToComputedFranklinBalance(token, amount: BigNumberish) {
        this.addToComputedBalance(this.computedFranklinBalances, token, amount);
    }

    // #endregion

    // #region construct
    private static id: number = 0;
    static async new(): Promise<LocalWallet> {
        let wallet = new LocalWallet(LocalWallet.id++);
        await wallet.prepare();
        return wallet;
    }

    private constructor(public id) {}

    private async prepare(): Promise<void> {
        let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id).connect(provider);
        this.franklinWallet = await Wallet.fromEthWallet(signer);        
        await this.franklinWallet.updateState();
        for (let i = 0; i < this.franklinWallet.supportedTokens.length; i++) {
            let token = this.franklinWallet.supportedTokens[i];
            if (this.franklinWallet.ethState.onchainBalances[token.id] !== undefined) {
                this.computedOnchainBalances[token.id] = bigNumberify(this.franklinWallet.ethState.onchainBalances[token.id]);
            }
            if (this.franklinWallet.ethState.contractBalances[token.id] !== undefined) {
                this.computedLockedBalances[token.id] = bigNumberify(this.franklinWallet.ethState.contractBalances[token.id]);
            }
            if (this.franklinWallet.franklinState.commited.balances[token.id] !== undefined) {
                this.computedFranklinBalances[token.id] = bigNumberify(this.franklinWallet.franklinState.commited.balances[token.id]);
            }
        }
    }

    // #endregion

    // #region realBalances

    onchainBalance(id: number): string {
        return this.franklinWallet.ethState.onchainBalances[id].toString();    
    }

    lockedBalance(id: number): string {
        return this.franklinWallet.ethState.contractBalances[id].toString();
    }

    franklinCommittedBalance(id: number): string {
        return (this.franklinWallet.franklinState.commited.balances[id] || bigNumberify(0)).toString();
    }

    // #endregion

    // #region info
    public async toJSON(): Promise<string> {
        return JSON.stringify({
            address: this.franklinWallet.address,
            balances: await this.getAllBalancesString(),
            actions: this.actions
        }, null, 4);
    }

    public async getBalanceForTokenAsString(token: Token): Promise<string[]> {
        await this.franklinWallet.updateState();
        let res: string[] = [];
        res.push(`for token(${token.id}) has computed`);
        res.push(
            `onchain: ${this.getComputedOnchainBalance(token)}, `
            + `locked: ${this.getComputedLockedBalance(token)}, `
            + `franklin: ${this.getComputedFranklinBalance(token)}`
            + ` and actual`
        );
        res.push(
            `onchain: ${this.onchainBalance(token.id)}`
            + `, locked: ${this.lockedBalance(token.id)}`
            + `, franklin: ${this.franklinCommittedBalance(token.id)}`
        );
        return res;
    }

    public async getAllBalancesString(): Promise<string[]> {
        await this.franklinWallet.updateState();
        let res: string[] = [];
        for (let i = 0; i < this.franklinWallet.supportedTokens.length; ++i) {
            let token = this.franklinWallet.supportedTokens[i];
            res = res.concat(await this.getBalanceForTokenAsString(token));
        }
        return res;
    }

    public async getWalletDescriptionString(): Promise<string> {
        let balances = await this.getAllBalancesString();
        return `${this.franklinWallet.address}\n${balances.join('\n')}`;
    }
    // #endregion

    // #region actions
    async withdraw(token: Token, amount: BigNumberish, fee: BigNumberish) {
        let offRes = await this.franklinWallet.widthdrawOffchain(token, amount, fee);
        if (offRes.err) throw new Error(offRes.err);
        let receipt = await this.franklinWallet.waitTxReceipt(offRes.hash);
        if (receipt.fail_reason) {
            throw new Error(receipt.fail_reason);
        }
        // await this.franklinWallet.widthdrawOnchain(token, amount);
        
        // async widthdrawOnchain(token: Token, amount: BigNumberish) {
        //     const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        //     if (token.id == 0) {
        //         const tx = await franklinDeployedContract.withdrawETH(amount, {gasLimit: 200000});
        //         await tx.wait(2);
        //         return tx.hash;
        //     } else {
        //         const tx = await franklinDeployedContract.withdrawERC20(token.address, amount, {gasLimit: bigNumberify("150000")});
        //         await tx.wait(2);
        //         return tx.hash;
        //     }
        // }

        // async widthdrawOffchain(token: Token, amount: BigNumberish, fee: BigNumberish) {
        //     let nonce = await this.getNonce();
        //     let tx = {
        //         type: 'Withdraw',
        //         account: this.address,
        //         eth_address: await this.ethWallet.getAddress(),
        //         token: token.id,
        //         amount: bigNumberify(amount).toString(),
        //         fee: bigNumberify(fee).toString(),
        //         nonce: nonce,
        //     };

        //     let res = await this.provider.submitTx(tx);
        //     return res;
        // }
    }

    public static async reason(hash: string): Promise<string> {
        const tx = await provider.getTransaction(hash);
        if (!tx) return "tx not found";

        const receipt = await provider.getTransactionReceipt(hash);
        
        if (receipt.status) return receipt.status.toString();

        const code = await provider.call(tx, tx.blockNumber);
        const reason = hex_to_ascii(code.substr(138));
        return reason;

        function hex_to_ascii(str1) {
            const hex  = str1.toString();
            let str = "";
            for (let n = 0; n < hex.length; n += 2) {
                str += String.fromCharCode(parseInt(hex.substr(n, 2), 16));
            }
            return str;
        }        
    }

    private async depositOnchain(token: Token, amount: BigNumber) {
        await this.franklinWallet.updateState();
        let res = await this.franklinWallet.deposit(token, amount);
        console.log('deposit onchain res');
        console.log(res);
        await this.franklinWallet.updateState();
    }

    async deposit(token: Token, amount: BigNumber, fee: BigNumber) {
        let total_amount = amount.add(fee);

        const zero = bigNumberify(0);
        const negative_amount = zero.sub(total_amount);
        
        if (this.getComputedOnchainBalance(token).gte(total_amount)) {
            // console.log("transaction should work");
            this.addToComputedOnchainBalance(token, negative_amount);
            this.addToComputedFranklinBalance(token, amount);
        }

        await this.depositOnchain(token, total_amount);
        await sleep(1000);
    }

    async sendTransaction(wallet2: LocalWallet, token: Token, amount: BigNumberish, fee: BigNumberish) {        
        amount = bigNumberify(amount);
        fee = bigNumberify(fee);
        const zero = bigNumberify(0);
        const total_amount = amount.add(fee);
        const negative_amount = zero.sub(total_amount);
        if (this.getComputedFranklinBalance(token).gte(total_amount)) {
            this.addToComputedFranklinBalance(token, negative_amount);
            wallet2.addToComputedFranklinBalance(token, amount);
        }

        let res = await this.franklinWallet.transfer(wallet2.franklinWallet.address, token, amount, fee);
        if (res.err) throw new Error(res.err);
        let receipt = await this.franklinWallet.waitTxReceipt(res.hash);
        if (receipt.fail_reason) throw new Error(receipt.fail_reason);
    }

    // #endregion
}
