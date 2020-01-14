import * as ethers from 'ethers';
import * as zksync from 'zksync';
import * as utils from './utils';
import { Token } from 'zksync/build/types';
import { sleep } from 'zksync/build/utils';
const contractCode = require('../../contracts/flat_build/Franklin');

const ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const ethProxy = new zksync.ETHProxy(ethersProvider, {
    govContract: process.env.GOVERNANCE_ADDR,
    mainContract: process.env.CONTRACT_ADDR,
});

export let syncProvider: zksync.Provider;
export let tokensInfo;
export let tokens;

const syncProviderPromise = (async () => {
    syncProvider = await zksync.Provider.newWebsocketProvider(process.env.WS_API_ADDR);
    tokensInfo = await syncProvider.getTokens();
    tokens = Object.keys(tokensInfo);
})();

const contract = new ethers.Contract(
    process.env.CONTRACT_ADDR, 
    contractCode.interface,
    ethersProvider,
);

export class WalletDecorator {
    syncNonce: number;
    ethNonce: number;

    constructor(
        public ethWallet, 
        public syncWallet
    ) {}

    static async isExodus() {
        return await contract.exodusMode();
    }

    static async waitExodus(action?) {
        while (await WalletDecorator.isExodus() == false) {
            await sleep(1000);
        }

        switch (action) {
            case 'print':
                console.log('📕 Enter exodus mode.');
                break;
            case undefined:
                break;
            default:
                throw new Error('switch reached default');
        }
    }

    static async waitReady() {
        await syncProviderPromise;
    }

    static async balancesToWithdraw(address, token: Number | Token) {
        const tokenId 
            = typeof token === 'string'
            ? (await syncProvider.getTokens())[token].id
            : token;

        return await contract.balancesToWithdraw(address, tokenId).then(ethers.utils.formatEther);
    }

    async balancesToWithdraw(token: (Number | Token)[]) {
        return Object.assign({}, 
            ...await Promise.all(
                tokens.map(
                    async token => ({
                        [token]: await WalletDecorator.balancesToWithdraw(this.ethWallet.address, token)
                    })
                )
            )
        );
    }

    async prettyPrintBalancesToWithdraw(tokens) {
        const balances = await this.balancesToWithdraw(tokens);
        for (const [token, balance] of Object.entries(balances)) {
            console.log(`Token: ${token}, withdraw: ${balance}`);
        }
    }

    static async fromPath(path: string) {
        const ethWallet = await ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, path).connect(ethersProvider);
        const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider, ethProxy);
        const wallet = new WalletDecorator(ethWallet, syncWallet);
        wallet.syncNonce = await syncWallet.getNonce();
        wallet.ethNonce = await ethWallet.getTransactionCount();
        console.log(`wallet ${syncWallet.address()} syncNonce ${wallet.syncNonce}, ethNonce ${wallet.ethNonce}`);
        return wallet;
    }

    static async fromId(id: number) {
        return WalletDecorator.fromPath(`m/44'/60'/0'/0/${id}`);
    }

    async deposit(amount, tokens) {
        return await Promise.all(
            tokens.map(async token => {
                const nonce = this.ethNonce++;
                const payload = {
                    depositFrom: this.ethWallet,
                    depositTo: this.syncWallet,
                    token: token,
                    amount: amount,
                    overrideOptions: {
                        nonce,
                    },
                };

                // console.log(`Deposit with ${nonce}`);
                const deposit = await zksync.depositFromETH(payload);
                // console.log(`Awaited deposit with ${nonce}`);
                const receipt = await deposit.awaitReceipt();
                // console.log(`Awaited receipt of deposit with ${nonce}`);
            })
            .map(promise => promise
                .catch(utils.jrpcErrorHandler("Deposit error"))
                .catch(console.log)
            )
        );
    }

    async transfer(wallet, amount, tokens) {
        const fee = ethers.utils.bigNumberify(0);
        return await Promise.all(
            tokens
            .map(async token => {
                const nonce = this.syncNonce++;
                const tx = await this.syncWallet.syncTransfer({
                    to: wallet.syncWallet.address(),
                    token,
                    amount,
                    fee,
                    nonce,
                });
                const receipt = await tx.awaitReceipt();
                console.log(`transfer ok ${nonce}`)
            })
            .map(promise => promise
                .catch(utils.jrpcErrorHandler("Transfer error"))
                .catch(console.log)
            )
        );
    }

    async withdraw(amount, tokens) {
        const fee = ethers.utils.bigNumberify(0);
        const ethAddress = await this.ethWallet.getAddress();
        return await Promise.all(
            tokens.map(
                async token => {
                    const nonce = this.syncNonce++;
                    const withdrawParams = {
                        ethAddress,
                        token,
                        amount,
                        fee,
                        nonce,
                    };
        
                    console.log("withdrawParams.nonce", withdrawParams.nonce);
                    const tx = await this.syncWallet.withdrawTo(withdrawParams);
                    console.log(`withdraw sent ${nonce} ${this.syncWallet.address()}`);
                    const receipt = await tx.awaitReceipt();
                    console.log(`withdraw succ ${nonce}`);
                }
            )
            .map(promise => promise
                .catch(utils.jrpcErrorHandler('Withdraw error'))
                .catch(console.log)
            )
        );
    }

    async prettyPrintBalances(tokens) {
        const ethAddress       = await this.ethWallet.getAddress();
        const syncAddress      = this.syncWallet.address();
        const withdrawBalances = await this.balancesToWithdraw(tokens);
        console.log(`Balance of ${ethAddress} ( ${syncAddress} ):`);
        console.table(
            Object.assign({},
                ...await Promise.all(
                    tokens.map(
                        async token => {
                            const eth      = await zksync.getEthereumBalance(this.ethWallet, token).then(ethers.utils.formatEther);
                            const sync     = await this.syncWallet.getBalance(token).then(ethers.utils.formatEther);
                            const withdraw = withdrawBalances[token];
                            return {
                                [token]: {
                                    eth,
                                    sync,
                                    withdraw,
                                },
                            };
                        }
                    )
                )
            )
        );
    }
}
