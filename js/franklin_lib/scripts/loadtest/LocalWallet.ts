import { AbstractOperation } from './AbstractOperation';
import { bigNumberify, BigNumber, BigNumberish } from 'ethers/utils';
import { ethers } from 'ethers';
import { Wallet, Token } from '../../src/wallet';

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

export class LocalWallet {
    public resetNonce(): void {
        this.franklinWallet.pendingNonce = null;
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
        const wallet = new LocalWallet(LocalWallet.id++);
        await wallet.prepare();
        return wallet;
    }

    private constructor(public id) {}

    private async prepare(): Promise<void> {
        const signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id).connect(provider);
        this.franklinWallet = await Wallet.fromEthWallet(signer);
        await this.franklinWallet.updateState();
        for (let i = 0; i < this.franklinWallet.supportedTokens.length; i++) {
            const token = this.franklinWallet.supportedTokens[i];
            if (this.franklinWallet.ethState.onchainBalances[token.id] !== undefined) {
                this.computedOnchainBalances[token.id] = bigNumberify(
                    this.franklinWallet.ethState.onchainBalances[token.id],
                );
            }
            if (this.franklinWallet.ethState.contractBalances[token.id] !== undefined) {
                this.computedLockedBalances[token.id] = bigNumberify(
                    this.franklinWallet.ethState.contractBalances[token.id],
                );
            }
            if (this.franklinWallet.franklinState.commited.balances[token.id] !== undefined) {
                this.computedFranklinBalances[token.id] = bigNumberify(
                    this.franklinWallet.franklinState.commited.balances[token.id],
                );
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
        return JSON.stringify(
            {
                address: this.franklinWallet.address,
                balances: await this.getAllBalancesString(),
                actions: this.actions,
            },
            null,
            4,
        );
    }

    public async getBalanceForTokenAsString(token: Token): Promise<string[]> {
        await this.franklinWallet.updateState();
        const res: string[] = [];
        res.push(`for token(${token.id}) has computed`);
        res.push(
            `onchain: ${this.getComputedOnchainBalance(token)}, ` +
                `locked: ${this.getComputedLockedBalance(token)}, ` +
                `franklin: ${this.getComputedFranklinBalance(token)}` +
                ` and actual`,
        );
        res.push(
            `onchain: ${this.onchainBalance(token.id)}` +
                `, locked: ${this.lockedBalance(token.id)}` +
                `, franklin: ${this.franklinCommittedBalance(token.id)}`,
        );
        return res;
    }

    public async getAllBalancesString(): Promise<string[]> {
        await this.franklinWallet.updateState();
        let res: string[] = [];
        for (let i = 0; i < this.franklinWallet.supportedTokens.length; ++i) {
            const token = this.franklinWallet.supportedTokens[i];
            res = res.concat(await this.getBalanceForTokenAsString(token));
        }
        return res;
    }

    public async getWalletDescriptionString(): Promise<string> {
        const balances = await this.getAllBalancesString();
        return `${this.franklinWallet.address.toString('hex')}\n${balances.join('\n')}`;
    }
    // #endregion

    // #region actions
    async withdraw(token: Token, amount: BigNumberish, fee: BigNumberish) {
        const res = await this.franklinWallet.widthdrawOffchain(token, amount, fee);

        if (res.err) throw new Error(res.err);

        let receipt = await this.franklinWallet.waitTxReceipt(res.hash);

        if (receipt.fail_reason) throw new Error(receipt.fail_reason);

        while (!receipt.verified) {
            receipt = await this.franklinWallet.waitTxReceipt(res.hash);
            await sleep(1000);
        }

        const tx_hash = await this.franklinWallet.widthdrawOnchain(token, amount);

        const status = await this.getOnchainTxStatus(tx_hash);

        if (status.success == false) throw new Error(status.message);
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
                message: 'Tx Success',
            };
        }

        const tx = await this.franklinWallet.ethWallet.provider.getTransaction(tx_hash);
        const code = await this.franklinWallet.ethWallet.provider.call(tx, tx.blockNumber);

        if (code == '0x') {
            return {
                success: false,
                message: 'Empty revert reason',
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

    private async depositOnchain(token: Token, amount: BigNumber) {
        await this.franklinWallet.updateState();
        const hash = await this.franklinWallet.deposit(token, amount);
        console.log('hash', hash);

        const status = await this.getOnchainTxStatus(hash);
        if (status.success == false) {
            throw new Error(status.message);
        }

        await this.franklinWallet.updateState();
    }

    async deposit(token: Token, amount: BigNumber, fee: BigNumber) {
        const total_amount = amount.add(fee);

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

        const res = await this.franklinWallet.transfer(wallet2.franklinWallet.address, token, amount, fee);
        if (res.err) throw new Error(res.err);
        const receipt = await this.franklinWallet.waitTxReceipt(res.hash);
        if (receipt.fail_reason) throw new Error(receipt.fail_reason);
    }

    // #endregion
}
