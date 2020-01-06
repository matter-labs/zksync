const ethers = require('ethers');
const zksync = require('zksync');
import * as utils from './utils';

const ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const ethProxy = new zksync.ETHProxy(ethersProvider, {
    govContract: process.env.GOVERNANCE_ADDR,
    mainContract: process.env.CONTRACT_ADDR,
});

export let syncProvider;
export let tokens;

const syncProviderPromise = (async () => {
    syncProvider = await zksync.Provider.newWebsocketProvider(process.env.WS_API_ADDR);
    tokens = await syncProvider.getTokens().then(Object.keys);
})();

export class WalletDecorator {
    nonce: number;

    constructor(
        public ethWallet, 
        public syncWallet
    ) {}

    static async waitReady() {
        await syncProviderPromise;
    }

    static async fromPath(path: string) {
        const ethWallet = await ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, path).connect(ethersProvider);
        const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider, ethProxy);
        const wallet = new WalletDecorator(ethWallet, syncWallet);
        wallet.nonce = await syncWallet.getNonce();
        console.log(`wallet ${syncWallet.address()} nonce ${wallet.nonce}`);
        return wallet;
    }
    
    static async fromId(id: number) {
        return WalletDecorator.fromPath(`m/44'/60'/0'/0/${id}`);
    }

    async deposit(amount, tokens) {
        const receipts = [];
        for (const token of tokens) {
            const deposit = await zksync.depositFromETH({
                depositFrom: this.ethWallet,
                depositTo: this.syncWallet,
                token: token,
                amount: amount,
            });
            receipts.push(await deposit.awaitReceipt());
        }
        return receipts;
    }

    async transfer(wallet, amount, tokens) {
        const fee = ethers.utils.bigNumberify(0);
        const promises = [];
        for (const token of tokens) {
            const promise = this
                .syncWallet
                .syncTransfer({
                    to: wallet.syncWallet.address(),
                    token,
                    amount: amount,
                    fee,
                    nonce: this.nonce++,
                })
                .then(tx => tx.awaitReceipt())
                .then((nonce => receipt => console.log(`transfer ok ${nonce}`))(this.nonce))
                .catch(utils.jrpcErrorHandler("Transfer error"))
                .catch(console.error);

            promises.push(promise);
        }
        return await Promise.all(promises);
    }

    async withdraw(amount, tokens) {
        const promises = [];
        const fee = ethers.utils.bigNumberify(0);
        const ethAddress = await this.ethWallet.getAddress();
        for (const token of tokens) {
            const withdrawParams = {
                ethAddress,
                token,
                amount,
                fee,
                nonce: this.nonce++,
            };

            console.log("withdrawParams.nonce", withdrawParams.nonce);

            const promise = this.syncWallet
                .withdrawTo(withdrawParams)
                .then((nonce => {
                    console.log(`withdraw sent ${nonce} ${this.syncWallet.address()}`);
                })(this.nonce))
                .then(tx => tx.awaitReceipt())
                .then((nonce => receipt => {
                    console.log(`withdraw succ ${nonce}`);
                })(this.nonce))
                .catch(utils.jrpcErrorHandler('Withdraw error'))
                .catch(console.error);
            
            promises.push(promise);
        }
        return await Promise.all(promises);
    }

    async prettyPrintBalances(tokens) {
        const ethAddress = await this.ethWallet.getAddress();
        const syncAddress = this.syncWallet.address();
        console.log(`Balance of ${ethAddress} ( ${syncAddress} ):`);
        for (const token of tokens) {
            const ethBalance  = await zksync.getEthereumBalance(this.ethWallet, token).then(ethers.utils.formatEther);
            const syncBalance = await this.syncWallet.getBalance(token).then(ethers.utils.formatEther);
            console.log(`Token: ${token}, eth: ${ethBalance}, sync: ${syncBalance}`);
        }
    }
}
