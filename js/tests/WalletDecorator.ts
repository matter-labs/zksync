import * as ethers from 'ethers';
import * as zksync from 'zksync';
import * as utils from './utils';
import { sleep } from 'zksync/build/utils';
import { bigNumberify } from 'ethers/utils';
const contractCode = require('../../contracts/build/Franklin');
const erc20ContractCode = require('openzeppelin-solidity/build/contracts/IERC20');

const ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const ethProxy = new zksync.ETHProxy(ethersProvider, {
    govContract: process.env.GOVERNANCE_ADDR,
    mainContract: process.env.CONTRACT_ADDR,
});

export let syncProvider;
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

export type TxResult = {
    payload: any,
    tx: any,
    receipt: any,
    error: any,
};

export class WalletDecorator {
    syncNonce: number;
    ethNonce: number;
    contract: ethers.Contract;

    constructor(
        public ethWallet,
        public syncWallet: zksync.Wallet,
    ) {
        this.contract = new ethers.Contract(
            process.env.CONTRACT_ADDR, 
            contractCode.interface,
            ethWallet,
        );
    }

    async callCancelOutstandingDepositsForExodusModeNTimes(n) {
        return await Promise.all(
            utils.rangearr(n).map(
                _ => this.cancelOutstandingDepositsForExodusMode(10, { gasLimit: 1000000 })
            )
        );
    }

    async cancelOutstandingDepositsForExodusMode(numDeposits = 10, ethTxOptions?) {
        const nonce = this.ethNonce++;
        const tx = await this.contract.cancelOutstandingDepositsForExodusMode(
            numDeposits, 
            { 
                nonce, 
                ...ethTxOptions 
            }
        );
        return tx.wait();
    }

    static async revertReason(hash) {
        const tx = await ethersProvider.getTransaction(hash);

        if (!tx) {
            return "tx not found";
        }

        const receipt = await ethersProvider.getTransactionReceipt(hash);
    
        if (receipt.status) {
            return "tx success";
        } 
        
        const code = await ethersProvider.call(tx, tx.blockNumber);
    
        if (code == '0x') {
            return 'empty revert reason';
        }
        
        return code
            .substr(138)
            .match(/../g)
            .map(h => parseInt(h, 16))
            .map(c => String.fromCharCode(c))
            .join('')
            .split('')
            .filter(c => /\w/.test(c))
            .join('');
    }

    static async isExodus() {
        return await contract.exodusMode();
    }

    static async waitExodus(action?) {
        while (await WalletDecorator.isExodus() == false) {
            await sleep(3000);
        }

        switch (action) {
            case 'print':
                console.log(`📕 it's exodus mode.`);
                break;
            case undefined:
                break;
            default:
                throw new Error('switch reached default');
        }
    }

    static async waitReady() {
        await syncProviderPromise;
        
        // https://github.com/ethers-io/ethers.js/issues/362
        await ethersProvider.getNetwork();
    }

    async prettyPrintBalancesToWithdraw(tokens) {
        const balances = await this.balancesToWithdraw(tokens);
        for (const [token, balance] of Object.entries(balances)) {
            console.log(`Token: ${token}, withdraw: ${balance}`);
        }
    }

    static async fromEthWallet(ethWallet) {
        const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);
        const wallet = new WalletDecorator(ethWallet, syncWallet);
        await wallet.resetNonce();
        console.log(`wallet ${syncWallet.address()} syncNonce ${wallet.syncNonce}, ethNonce ${wallet.ethNonce}`);
        return wallet;
    }

    static async fromPath(path, mnemonic = process.env.TEST_MNEMONIC) {
        const ethWallet = await ethers.Wallet.fromMnemonic(mnemonic, path).connect(ethersProvider);
        return await WalletDecorator.fromEthWallet(ethWallet);
    }

    static async fromId(id) {
        return await WalletDecorator.fromPath(`m/44'/60'/0'/0/${id}`);
    }

    async resetNonce() {
        this.syncNonce = await this.syncWallet.getNonce();
        this.ethNonce = await this.ethWallet.getTransactionCount();
    }

    async setCurrentPubkeyWithZksyncTx() {
        if (await this.syncWallet.isSigningKeySet()) return;

        const startTime = new Date().getTime();
        await (await this.syncWallet.onchainAuthSigningKey(this.syncNonce)).wait();
        const changePubkeyHandle = await this.syncWallet.setSigningKey(this.syncNonce++, true);
        console.log(`Change pubkey onchain posted: ${(new Date().getTime()) - startTime} ms`);
        await changePubkeyHandle.awaitReceipt();
        console.log(`Change pubkey onchain committed: ${(new Date().getTime()) - startTime} ms`);
    }

    async mainchainSendToMany(wallets, tokens, amounts) {
        const promises = [];
        for (const [wallet, token, amount] of utils.product(wallets, tokens, amounts)) {
            promises.push(
                this.mainchainSend(wallet, token, amount)
            );
        }
        return await Promise.all(promises);
    }

    async mainchainSend(wallet, token, amount) {
        if (token == 'ETH') {
            let nonce = this.ethNonce;
            this.ethNonce += 1;
            const tx = await this.ethWallet.sendTransaction({
                to: wallet.ethWallet.address,
                value: amount,
                nonce: nonce++,
            });
            return await tx.wait();
        } else {
            let nonce = this.ethNonce;
            this.ethNonce += 2;
            const erc20contract = new ethers.Contract(
                token,
                erc20ContractCode.abi,
                this.ethWallet
            );
            const approveTx = await erc20contract.approve(
                wallet.ethWallet.address,
                amount,
                {
                    nonce: nonce++,
                },
            );
            const tx = await erc20contract.transfer(
                wallet.ethWallet.address,
                amount,
                {
                    nonce: nonce++,
                },
            );
            return await tx.wait();
        }
    }

    async emergencyWithdraw(tokens): Promise<TxResult[]> {
        return await Promise.all(
            tokens.map(async token => {
                const ethNonce = this.ethNonce++;
                const syncNonce = this.syncNonce++;
                
                let payload, tx, receipt;
                let error = null;
                try {
                    payload = {
                        withdrawTo: this.ethWallet,
                        withdrawFrom: this.syncWallet,
                        token,
                        nonce: syncNonce,
                        ethTxOptions: {
                            nonce: ethNonce,
                        },
                    };
                    tx = await this.syncWallet.emergencyWithdraw(payload);
                    receipt = tx.awaitReceipt();
                } catch (e) {
                    error = e;
                }

                return {
                    payload,
                    tx,
                    receipt,
                    error,
                };
            })
        );
    }

    async deposit(amount, tokens): Promise<TxResult[]> {
        return await Promise.all(
            tokens.map(async token => {
                const nonce = this.ethNonce;
                this.ethNonce += token == 'ETH' ? 1 : 2;

                let payload, tx, receipt;
                let error = null;
                try {
                    payload = {
                        depositTo: this.syncWallet.address(),
                        token,
                        amount,
                        ethTxOptions: {
                            nonce,
                        },
                    };
                    tx = await this.syncWallet.depositToSyncFromEthereum(payload);
                    receipt = await tx.awaitReceipt();
                } catch (e) {
                    error = e;
                }

                return {
                    payload,
                    tx,
                    receipt,
                    error,
                };
            })
        );
    }

    async transfer(wallet, amount, tokens): Promise<TxResult[]> {
        const fee = ethers.utils.bigNumberify(0);
        return await Promise.all(
            tokens
            .map(async token => {
                const nonce = this.syncNonce++;

                let payload, tx, receipt;
                let error = null;
                try {
                    payload = {
                        to: wallet.syncWallet.address(),
                        token,
                        amount,
                        fee,
                        nonce,
                    };
                    tx = await this.syncWallet.syncTransfer(payload);
                    receipt = await tx.awaitReceipt();
                } catch (e) {
                    error = e;
                }

                return {
                    payload,
                    tx,
                    receipt,
                    error,
                };
            })
        );
    }

    async withdraw(amount, tokens): Promise<TxResult[]> {
        const fee = ethers.utils.bigNumberify(0);
        const ethAddress = await this.ethWallet.getAddress();
        return await Promise.all(
            tokens.map(
                async token => {
                    const nonce = this.syncNonce++;

                    let payload, tx, receipt;
                    let error = null;
                    try {
                        payload = {
                            ethAddress,
                            token,
                            amount,
                            fee,
                            nonce,
                        };
                        tx = await this.syncWallet.withdrawFromSyncToEthereum(payload);
                        receipt = await tx.awaitReceipt();
                    } catch (e) {
                        error = e;
                    }
                    return {
                        payload,
                        tx,
                        receipt,
                        error,
                    };
                }
            )
        );
    }

    static async balancesToWithdraw(address, token) {
        const tokenId 
            = typeof token === 'string'
            ? tokensInfo[token].id
            : token;

        return (await contract.balancesToWithdraw(address, tokenId)).balanceToWithdraw.then(ethers.utils.formatEther);
    }

    async balancesToWithdraw(tokens) {
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

    async balances(tokens) {
        const withdrawBalances = await this.balancesToWithdraw(tokens);
        return Object.assign({},
            ...await Promise.all(
                tokens.map(
                    async token => {
                        const eth      = await this.syncWallet.getEthereumBalance(token).then(ethers.utils.formatEther);
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
    }

    async prettyPrintBalances(tokens) {
        const syncAddress      = this.syncWallet.address();
        console.log(`Balance of ${syncAddress}:`);
        console.table(await this.balances(tokens));
    }
}
