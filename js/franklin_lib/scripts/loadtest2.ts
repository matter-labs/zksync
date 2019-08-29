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
        return mined;
    }
}

class LocalWallet {
    public wallet: Wallet;
    history = [];

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
    }

    onchainBalance(id) {
        return this.wallet.ethState.onchainBalances[id].toString();    
    }

    lockedBalance(id) {
        return this.wallet.ethState.contractBalances[id].toString();
    }

    async depositOnchain(token: Token, amount: BigNumber) {
        await this.wallet.updateState();
        await this.wallet.depositOnchain(token, amount);
        await this.wallet.updateState();
    }

    async depositOffchain(token: Token, amount: BigNumber, fee: BigNumber) {
        await this.wallet.depositOffchain(token, amount, fee);
    }

    async depositOld() {
        await this.wallet.updateState();

        let token = this.wallet.supportedTokens[0];
        let amount = ethers.utils.bigNumberify('1000');
        let fee = ethers.utils.bigNumberify("0");

        await this.wallet.depositOnchain(token, amount);
        await this.wallet.updateState();

        let bnAmount = new BN(amount.toString(), 10);
        let bnFee = new BN(fee.toString(), 10);

        // console.log("WAITING PROVING");
        // await this.wallet.waitProved();

        do {
            await this.wallet.updateState();
            await sleep(1000);
        } while ( ! this.wallet.ethState.contractBalances[0]);
        
        console.log(`${this.wallet.address} contract: ${this.wallet.ethState.contractBalances[0]}`);

    }

    async deposit(token: Token, amount: BigNumber, fee: BigNumber) {
        let feeless_amount = amount.sub(fee);

        await this.depositOnchain(token, amount);
        await this.depositOffchain(token, feeless_amount, fee);
    }
}

const INIT_NUM_WALLETS = 3;
const NEW_WALLET_PROB = 0.05;
const BATCH_SIZE = 10;
const NUM_BATCHES = 3;

let prando = new Prando();
let richWallet = null;
let tokens = null;
let commonWallets = [];

let Utils = {
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
        return bigNumberify(prando.nextInt(from, to).toString());
    },

    selectRandomToken: function() {
        return tokens[0]; // TODO
        // return prando.nextArrayItem(tokens);
    },

    getGoroutineId: ((counter) => () => String(++counter).padStart(3, '0'))(0),
}

let Actions = {
    'create_new': function() {
        return async () => {
            console.log('create_new')
            commonWallets.push( await LocalWallet.new(richWallet) );
        };
    },

    'deposit': function() {
        let goroutineId = Utils.getGoroutineId();
        let wallet = Utils.selectRandomWallet();
        // let token = Utils.selectRandomToken();
        let token = tokens[0];
        let amount = Utils.selectRandomAmount(0, 1000);
        let fee = Utils.selectRandomAmount(0, 10);
        return async () => {
            let message = null;
            try {
                console.log(`${goroutineId} ### trying deposit`);
                wallet.history.push(`depositing token(${token.id}) `
                    + `amount(${amount.toString()}) `
                    + `fee(${fee.toString()})`);

                await wallet.deposit(token, amount, fee);
                await wallet.wallet.updateState();
                
                message = `>>> deposit succeded for wallet ${wallet.wallet.address}, `
                    + `locked amount ${wallet.lockedBalance(token.id)}`;
            } catch (err) {
                let err_message = err.message;
                if (err_message.includes('insufficient funds')) {
                    await wallet.wallet.updateState();
                    err_message += `, onchain: ${wallet.onchainBalance(token.id)}`;
                    err_message += `, locked: ${wallet.lockedBalance(token.id)}`;
                    err_message += `, amount: ${amount.toString()}`;
                    err_message += `, fee: ${fee.toString()}`;
                }
                message = `<<< deposit failed for wallet ${wallet.wallet.address} with ${err_message}`;
            } finally {
                wallet.history.push(message);
                console.log(`${goroutineId} ${message}`);
                if (message.slice(0, 3) === '<<<') {
                    console.log(`history of wallet: ${wallet.history.join(' \n ')}`);
                }
            }
        };
    },

    'receive_money': function() {
        let goroutineId = Utils.getGoroutineId();
        let wallet = Utils.selectRandomWallet();
        let token = Utils.selectRandomToken();
        // let amount = Utils.selectRandomAmount(10000000000000, 100000000000000);
        let amount = bigNumberify('100000000000000000000000000000000')
        return async () => {
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
        };
    },

    'transfer': function() {
        let [w1, w2] = Utils.selectTwoRandomDistinctWallets();
        let amount = Utils.selectRandomAmount(1000, 10000);
        let fee = Utils.selectRandomAmount(0, 1000);
        return async () => {
            // console.log('transfer');
            // w1.sendTransaction(w2)
        };
    },
}

function getRandomAction() {
    let all_actions = Object.keys(Actions);
    let action = prando.nextArrayItem(all_actions);
    return Actions[action]()();
}

async function test() {
    richWallet = await RichEthWallet.new();
    
    for (let i = 0; i < INIT_NUM_WALLETS; i++)
        await (Actions.create_new()());

    tokens = commonWallets[0].wallet.supportedTokens;

    for (let i = 0; i < NUM_BATCHES; i++) {
        let batch = [];
        for (let j = 0; j < BATCH_SIZE; j++) {
            batch.push( getRandomAction() );
        }
        await Promise.all(batch);
    }
}

test()