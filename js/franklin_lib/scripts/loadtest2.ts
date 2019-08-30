import BN = require('bn.js');
import { Wallet, Token } from '../src/wallet';
import { ethers } from 'ethers';
import {bigNumberify, parseEther, BigNumber, BigNumberish} from "ethers/utils";
import Prando from 'prando';
import * as assert from 'assert';

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

class RichEthWallet {
    source: ethers.Wallet;
    sourceNonce: number;

    static async new() {
        let wallet = new RichEthWallet();
        await wallet.prepare();
        return wallet;
    }

    private constructor() {
        this.source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    }

    private async prepare() {
        this.sourceNonce = await this.source.getTransactionCount("pending")
    }

    async sendSome(wallet: LocalWallet, amount: BigNumberish) {
        let to = wallet.wallet.ethWallet;
        let txAddr = await to.getAddress();
        let txAmount = amount;
        let txNonce = this.sourceNonce
        
        ++this.sourceNonce;
        
        let promiseToSend = this.source.sendTransaction({
            to:     txAddr,
            value:  txAmount,
            nonce:  txNonce,
        });
        
        let mining = await promiseToSend;
        let mined = await mining.wait();
        console.log(`${txAddr} onchain ${await to.provider.getBalance(txAddr)}`);

        wallet.addToComputedOnchainBalance(tokens[0], txAmount);
        console.log(wallet.computedOnchainBalances);

        return mined;
    }
}

class LocalWallet {
    public wallet: Wallet;
    history = [];
    pendingActions = [];

    computedOnchainBalances = {};
    computedLockedBalances = {};
    computedFranklinBalances = {};

    static getComputedBalance(dict, token) {
        if (dict[token.id] === undefined) {
            dict[token.id] = bigNumberify(0);
        }
        return dict[token.id];
    }
    getComputedOnchainBalance(token) {
        return LocalWallet.getComputedBalance(this.computedOnchainBalances, token);
    }
    getComputedLockedBalance(token) {
        return LocalWallet.getComputedBalance(this.computedLockedBalances, token);
    }
    getComputedFranklinBalance(token) {
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

    static id = 0;
    static async new(richEthWallet: RichEthWallet) {
        let wallet = new LocalWallet(LocalWallet.id++);
        await wallet.prepare(richEthWallet);
        return wallet;
    }

    private constructor(public id) {}

    private async prepare(fundFranklin: RichEthWallet) {
        let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id).connect(provider);
        this.wallet = await Wallet.fromEthWallet(signer);        
        await this.wallet.updateState();
        for (let i = 0; i < this.wallet.supportedTokens.length; i++) {
            let token = this.wallet.supportedTokens[i];
            if (this.wallet.ethState.onchainBalances[token.id] !== undefined) {
                this.computedOnchainBalances[token.id] = this.wallet.ethState.onchainBalances[token.id];
            }
            if (this.wallet.ethState.contractBalances[token.id] !== undefined) {
                this.computedLockedBalances[token.id] = this.wallet.ethState.contractBalances[token.id];
            }
            if (this.wallet.franklinState.commited.balances[token.id] !== undefined) {
                this.computedFranklinBalances[token.id] = bigNumberify(this.wallet.franklinState.commited.balances[token.id]);
            }
        }
    }

    onchainBalance(id) {
        return this.wallet.ethState.onchainBalances[id].toString();    
    }

    lockedBalance(id) {
        return this.wallet.ethState.contractBalances[id].toString();
    }

    franklinCommittedBalance(id) {
        return this.wallet.franklinState.commited.balances[id];
    }

    private async depositOnchain(token: Token, amount: BigNumber) {
        await this.wallet.updateState();
        let res = await this.wallet.depositOnchain(token, amount);
        await this.wallet.updateState();
    }

    private async depositOffchain(token: Token, amount: BigNumber, fee: BigNumber) {
        let res = await this.wallet.depositOffchain(token, amount, fee);
        if (res.err) {
            throw new Error(res.err);
        }
    }

    async deposit(token: Token, amount: BigNumber, fee: BigNumber) {
        // if (token.address)
        // if we have less 
        let total_amount = amount.add(fee);
        if (this.getComputedOnchainBalance(token).lt(total_amount)) {
            console.log(`I don't send this stuff`);
            return;
        }

        // after the feat, 
        const zero = bigNumberify(0);
        const negative_amount = zero.sub(total_amount);
        if (false == zero.gte(negative_amount)) throw new Error('assertion failed');
        this.addToComputedOnchainBalance(token, negative_amount);
        
        let feeless_amount = amount.sub(fee);
        this.addToComputedFranklinBalance(token, feeless_amount);

        await this.depositOnchain(token, amount);
        await this.depositOffchain(token, feeless_amount, fee);
    }
}

const INIT_NUM_WALLETS = 1;
const NEW_WALLET_PROB = 0.00;
const BATCH_SIZE = 3;

let prando = new Prando();
let richWallet = null;
let tokens = null;
let commonWallets = [];

let Utils = {
    addNewWallet: async function() {
        console.log(`create_new`)
        commonWallets.push( await LocalWallet.new(richWallet) );
    },

    selectRandomWallet: function() {
        return prando.nextArrayItem(commonWallets);
    },
    
    selectTwoRandomDistinctWallets: function() {
        if (commonWallets.length < 2) throw new Error('there is no two wallets.');
        let w1 = Utils.selectRandomWallet();
        do {
            var w2 = Utils.selectRandomWallet();
        } while (w1 === w2);
        return [w1, w2];
    },
    
    selectRandomAmount: function(from: number, to: number) {
        // TODO
        return bigNumberify(to);
        // return bigNumberify(prando.nextInt(from, to).toString());
    },

    selectRandomToken: function() {
        return tokens[0]; // TODO
        // return prando.nextArrayItem(tokens);
    },

    getGoroutineId: (counter => () => String(++counter).padStart(3, '0'))(0),
}

let Actions = {
    'deposit': function(kwargs?) {
        let goroutineId = Utils.getGoroutineId();
        kwargs = kwargs || {};
        let wallet = kwargs.wallet || Utils.selectRandomWallet();
        let token  = kwargs.token  || Utils.selectRandomToken();
        let amount = kwargs.amount || Utils.selectRandomAmount(0, 1000);
        let fee    = kwargs.fee    || Utils.selectRandomAmount(0, 1000);
        wallet.pendingActions.push(async () => {
            let message = null;
            try {
                console.log(`${goroutineId} ### trying deposit`);
                wallet.history.push(`depositing token(${token.id}) `
                    + `amount(${amount.toString()}) `
                    + `fee(${fee.toString()})`);

                await wallet.deposit(token, amount, fee);
                await wallet.wallet.updateState();
                
                message = `>>> deposit succeded for wallet ${wallet.wallet.address}`
                    + `, onchain: ${wallet.onchainBalance(token.id)}`
                    + `, locked: ${wallet.lockedBalance(token.id)}`
                    + `, amount: ${amount.toString()}`
                    + `, fee: ${fee.toString()}`
                    + `, franklin: ${wallet.franklinCommittedBalance(token.id)}`;
            } catch (err) {
                let err_message = err.message;
                if (err_message.includes('insufficient funds')) {
                    await wallet.wallet.updateState();
                    err_message += `, onchain: ${wallet.onchainBalance(token.id)}`;
                    err_message += `, locked: ${wallet.lockedBalance(token.id)}`;
                    err_message += `, amount: ${amount.toString()}`;
                    err_message += `, fee: ${fee.toString()}`;
                    err_message += `, franklin: ${wallet.franklinCommittedBalance(token.id)}`;
                }
                message = `<<< deposit failed for wallet ${wallet.wallet.address} with ${err_message}`;
            } finally {
                wallet.history.push(message);
                console.log(`${goroutineId} ${message}`);
                if (message.slice(0, 3) === '<<<') {
                    console.log(`history of wallet: ${wallet.history.join(' \n ')}`);
                }
            }
        });
    },

    'receive_money': function(kwargs?) {
        let goroutineId = Utils.getGoroutineId();
        kwargs = kwargs || {};
        kwargs = kwargs || {};
        let wallet = kwargs.wallet || Utils.selectRandomWallet();
        let token  = kwargs.token  || Utils.selectRandomToken();
        let amount = kwargs.amount || Utils.selectRandomAmount(0, 1000000000000000);
        wallet.pendingActions.push(async () => {
            let message = null;
            try {
                console.log(`${goroutineId} ### trying receive money`);
                await richWallet.sendSome(wallet, amount);
                message = `>>> receive money succeded for wallet ${wallet.wallet.address}`;
            } catch (err) {
                message = `<<< receive money failed for wallet ${wallet.wallet.address} with ${err.message}`;
            } finally {
                wallet.history.push(message);
                console.log(`${goroutineId} ${message}`);
            }
        });
    },

    // 'transfer': function() {
    //     let [w1, w2] = Utils.selectTwoRandomDistinctWallets();
    //     let amount = Utils.selectRandomAmount(1000, 10000);
    //     let fee = Utils.selectRandomAmount(0, 1000);
    //     return async () => {
    //         // console.log('transfer');
    //         // w1.sendTransaction(w2)
    //     };
    // },
}

async function addRandomPendingActionToWallet(kwargs?) {
    kwargs = kwargs || {};
    if (Math.random() < NEW_WALLET_PROB) {
        await Utils.addNewWallet();
    }
    let all_actions = Object.keys(Actions);
    let action = prando.nextArrayItem(all_actions);
    Actions[action](kwargs);
}

async function test() {
    richWallet = await RichEthWallet.new();
    
    for (let i = 0; i < INIT_NUM_WALLETS; i++)
        await Utils.addNewWallet();

    tokens = commonWallets[0].wallet.supportedTokens;

    commonWallets.forEach(w => w.pendingActions = []);

    // for (let j = 0; j < BATCH_SIZE; j++) {
    //     await addRandomPendingActionToWallet();
    // }
    let wallet = commonWallets[0];
    // Actions.receive_money({wallet, amount: bigNumberify('10000000000000000000')});
    Actions.deposit({wallet, amount: bigNumberify('1000'), fee: bigNumberify('10')});
    Actions.deposit({wallet, amount: bigNumberify('1000'), fee: bigNumberify('10')});
    // Actions.deposit({wallet, amount: bigNumberify('1000'), fee: bigNumberify('1000')});
    Actions.deposit({wallet, amount: bigNumberify('1000'), fee: bigNumberify('10')});
    Actions.deposit({wallet, amount: bigNumberify('1000'), fee: bigNumberify('10')});
    Actions.deposit({wallet, amount: bigNumberify('1000'), fee: bigNumberify('1000')});

    
    await Promise.all(commonWallets.map(async w => {
        for (let i = 0; i < w.pendingActions.length; i++) {
            await w.pendingActions[i]();
        }
    }));
    
    commonWallets.forEach(wallet => {
        let token = tokens[0];
        console.log(`wallet ${wallet.wallet.address} has computed `
            + `onchain ${wallet.getComputedOnchainBalance(token)}, `
            + `locked ${wallet.getComputedLockedBalance(token)}, `
            + `franklin ${wallet.getComputedFranklinBalance(token)}, `
            + `and actual `
            + `onchain: ${wallet.onchainBalance(token.id)}`
            + `, locked: ${wallet.lockedBalance(token.id)}`
            + `, franklin: ${wallet.franklinCommittedBalance(token.id)}`
            + `, and its history is ${wallet.history.join(' \n ')}`);
    });
}

test()
