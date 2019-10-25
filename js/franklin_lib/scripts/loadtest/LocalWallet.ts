import { AbstractOperation } from './AbstractOperation'
import { bigNumberify, BigNumber, BigNumberish, parseEther } from "ethers/utils";
import { ethers, Contract } from 'ethers';
import { Wallet, Token, Address } from '../../src/wallet';
const IERC20Conract = require('openzeppelin-solidity/build/contracts/ERC20Mintable.json');

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

function bignumberifyOrZero(bal) {
    return bal == undefined 
        ? bigNumberify(0) 
        : bigNumberify(bal);
}

export class LocalWallet {
    // #region construct
    private franklinWallet: Wallet;
    private actions: AbstractOperation[] = [];
    
    private franklinCommitedBalances = {};
    private franklinVerifiedBalances = {};
    private contractBalances = {};
    private onchainBalances = {};

    private computedFranklinCommitedBalances = {};
    private computedFranklinVerifiedBalances = {};
    private computedContractBalances = {};
    private computedOnchainBalances = {};
    
    private constructor(public id) {}
    private static id: number = 0;
    
    public static async new(): Promise<LocalWallet> {
        let wallet = new LocalWallet(LocalWallet.id++);
        await wallet.prepare();
        return wallet;
    }
    
    public ethAddress: string;
    public address: string;
    private async prepare(): Promise<void> {
        let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id).connect(provider);
        this.franklinWallet = await Wallet.fromEthWallet(signer);
        
        this.ethAddress = await signer.getAddress();
        this.address = this.franklinWallet.address.toString();

        await this.updateState();

        this.computedFranklinCommitedBalances = Object.assign({}, this.franklinCommitedBalances);
        this.computedFranklinVerifiedBalances = Object.assign({}, this.franklinVerifiedBalances);
        this.computedContractBalances         = Object.assign({}, this.contractBalances);
        this.computedOnchainBalances          = Object.assign({}, this.onchainBalances);
    }

    public async updateState() {
        let ethState = await this.franklinWallet.getOnchainBalances();
        let franklinState = await this.franklinWallet.getAccountState();

        for (let token of await this.franklinWallet.provider.getTokens()) {
            this.onchainBalances[token.id]          = bignumberifyOrZero(ethState.onchainBalances[token.id]);
            this.contractBalances[token.id]         = bignumberifyOrZero(ethState.contractBalances[token.id])
            this.franklinCommitedBalances[token.id] = bignumberifyOrZero(franklinState.commited.balances[token.id])
            this.franklinVerifiedBalances[token.id] = bignumberifyOrZero(franklinState.verified.balances[token.id])
            this.updateComputedOnchainBalanceIfCloseToActual(token);
        }
    }

    private updateComputedOnchainBalanceIfCloseToActual(token: Token) {
        if (this.computedOnchainBalances[token.id] == undefined) return;

        let diff = this.computedOnchainBalances[token.id].sub(this.onchainBalances[token.id]);
        
        let zero = bigNumberify(0);
        if (diff.lt(zero)) {
            diff = zero.sub(diff);
        }

        if (diff.lt('1000000')) {
            this.computedOnchainBalances[token.id] = this.onchainBalances[token.id];
        }
    }

    // #endregion

    // #region unclassified
    private addComputedDict(dict, token: Token, amount: BigNumber) { dict[token.id] = dict[token.id].add(amount); }
    private addComputedOnchain              (token: Token, amount: BigNumber) { this.addComputedDict(this.computedOnchainBalances,          token, amount); }
    private addComputedContract             (token: Token, amount: BigNumber) { this.addComputedDict(this.computedContractBalances,         token, amount); }
    private addComputedCommitedFranklin     (token: Token, amount: BigNumber) { this.addComputedDict(this.computedFranklinCommitedBalances, token, amount); }
    private addComputedVerifiedFranklin     (token: Token, amount: BigNumber) { this.addComputedDict(this.computedFranklinVerifiedBalances, token, amount); }
    private subtractComputedOnchain         (token: Token, amount: BigNumber) { this.addComputedOnchain         (token, bigNumberify(0).sub(amount)); }
    private subtractComputedContract        (token: Token, amount: BigNumber) { this.addComputedContract        (token, bigNumberify(0).sub(amount)); }
    private subtractComputedCommitedFranklin(token: Token, amount: BigNumber) { this.addComputedCommitedFranklin(token, bigNumberify(0).sub(amount)); }
    private subtractComputedVerifiedFranklin(token: Token, amount: BigNumber) { this.addComputedVerifiedFranklin(token, bigNumberify(0).sub(amount)); }

    public getActions(): AbstractOperation[] { return this.actions; }
    public addAction(op: AbstractOperation): void { this.actions.push(op); }
    public resetNonce(): void { this.franklinWallet.pendingNonce = null; }
    // #endregion

    // #region get stuff
    public async toJSON(): Promise<string> {
        return JSON.stringify({
            address: this.franklinWallet.address,
            balances: await this.getAllBalancesString(),
            actions: this.actions
        }, null, 4);
    }

    private async getTransactionFee(tx_hash: string) {
        let receipt = await this.franklinWallet.ethWallet.provider.getTransactionReceipt(tx_hash);
        console.log(receipt.gasUsed);
    }

    private async getOnchainTxStatus(tx_hash: string) {
        let receipt = null;
        for (let i = 1; i <= 5 && !receipt; i++) {            
            receipt = await this.franklinWallet.ethWallet.provider.getTransactionReceipt(tx_hash);
            
            if (receipt) break;
            await sleep(4000);
        }

        if (receipt.status) {
            return {
                success: true,
                message: "Tx Success",
            };
        }

        const tx = await this.franklinWallet.ethWallet.provider.getTransaction(tx_hash);
        const code = await this.franklinWallet.ethWallet.provider.call(tx, tx.blockNumber);

        if (code == '0x') {
            return {
                success: false,
                message: "Empty revert reason",
            };
        }
        
        const reason = code
            .substr(138)
            .match(/../g)
            .map(h => parseInt(h, 16))
            .map(x => String.fromCharCode(x))
            .join('');
        
        return {
            success: false,
            message: `Revert reason is: ${reason}`,
        };
    }

    public getBalanceForTokenAsString(token: Token): string[] {
        let res: string[] = [];
        res.push(`for token(${token.id}) has computed\n`);
        res.push(
            `onchain: ${this.computedOnchainBalances[token.id]}, `
            + `locked: ${this.computedContractBalances[token.id]}, `
            + `franklin: ${this.computedFranklinCommitedBalances[token.id]}`
        );
        res.push(` and actual\n`);
        res.push(
            `onchain: ${this.onchainBalances[token.id]}, `
            + `locked: ${this.contractBalances[token.id]}, `
            + `franklin: ${this.franklinCommitedBalances[token.id]}`
        );
        return res;
    }

    public async getAllBalancesString(): Promise<string[]> {
        return Promise.all(
            (await this.franklinWallet.provider.getTokens())
            .map(token => this.getBalanceForTokenAsString(token))
        );
    }

    public async getWalletDescriptionString(): Promise<string> {
        let balances = await this.getAllBalancesString();
        return `${this.franklinWallet.address.toString()}\n${balances.join('\n')}`;
    }
    // #endregion

    // #region actions
    public async receiveMoney(from: ethers.Wallet, token: Token, amount: BigNumber, nonce: number) {
        if (token.id == 0) {
            let tx = await from.sendTransaction({
                to:     this.ethAddress,
                value:  amount,
                nonce:  nonce,
                gasLimit: bigNumberify("1500000"),
            });
            
            let mined = await tx.wait(2);
        } else {
            const contract = new Contract(token.address, IERC20Conract.abi, from);
            let tx = await contract.transfer(this.ethAddress, amount, { nonce, gasLimit: bigNumberify("1500000") });
            await tx.wait(2);
        }

        this.addComputedOnchain(token, amount);
    }

    public async withdraw(token: Token, amount: BigNumber, fee: BigNumber) {
        let total_amount = amount.add(fee);
        if (this.computedFranklinCommitedBalances[token.id].ge(total_amount)) {
            this.subtractComputedCommitedFranklin(token, total_amount);
        }

        let handle = await this.franklinWallet.widthdrawOffchain(token, amount, fee);
        handle.waitVerify();
        let handle2 = await this.franklinWallet.widthdrawOnchain(token, amount);
        handle2.wait(2);
    }

    public async deposit(token: Token, amount: BigNumber, fee: BigNumber) {
        if (this.computedOnchainBalances[token.id].gte(amount)) {
            this.subtractComputedOnchain(token, amount);
            this.addComputedCommitedFranklin(token, amount);
        }

        let handle = await this.franklinWallet.deposit(token, amount, fee);
        await handle.waitTxMine();
        return handle;
    }

    public async sendTransaction(wallet2: LocalWallet, token: Token, amount: BigNumber, fee: BigNumber) {        
        let total_amount = amount.add(fee);
        if (this.computedFranklinCommitedBalances[token.id].gte(total_amount)) {
            this.subtractComputedCommitedFranklin(token, total_amount);
            wallet2.addComputedCommitedFranklin(token, amount);
        }

        let handle = await this.franklinWallet.transfer(wallet2.franklinWallet.address, token, amount, fee);
        await handle.waitCommit();
    }

    // #endregion
}
