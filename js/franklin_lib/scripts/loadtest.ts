import BN = require('bn.js');
import { Wallet } from '../src/wallet';
import { ethers } from 'ethers';
import {bigNumberify, parseEther, BigNumber} from "ethers/utils";
import Prando from 'prando';
import * as assert from 'assert';

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

class RichEthWallet {
    source: ethers.Wallet;
    sourceNonce: number;
    
    constructor() {
        this.source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    }

    async prepare() {
        this.sourceNonce = await this.source.getTransactionCount("pending")
    }

    async sendSome(to: ethers.Signer, amount) {
        let txAddr = await to.getAddress();
        let txAmount = ethers.utils.parseEther(amount);
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

const DEPOSIT_AMOUNT = "100000000000";

class LocalWallet {
    public wallet: Wallet;

    constructor(public id) {}

    async prepare(fundFranklin: RichEthWallet) {
        let fundAmount = "100000";

        let signer = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/3/" + this.id).connect(provider);
        this.wallet = await Wallet.fromEthWallet(signer);
        // console.log(`created wallet ${this.id}`)
        
        await this.wallet.updateState();
        // console.log(`updated state ${this.id}`)
        // assert(this.wallet.ethState.contractBalances[0] === DEPOSIT_AMOUNT);
    }

    async deposit() {
        await this.wallet.updateState();

        let token = this.wallet.supportedTokens[0];
        let amount = ethers.utils.bigNumberify(DEPOSIT_AMOUNT);
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

    async depositOffchain() {
        let token = this.wallet.supportedTokens[0];
        let amount = ethers.utils.bigNumberify(DEPOSIT_AMOUNT);
        let fee = ethers.utils.bigNumberify("0");
        let bnAmount = new BN(amount.toString(), 10);
        let bnFee = new BN(fee.toString(), 10);

        console.log(await this.wallet.depositOffchain(token, amount, fee));
        await this.wallet.updateState();
        await this.wallet.waitPendingTxsExecuted();
        await this.wallet.updateState();

    }
}

const NUM_WALLETS = 10;

async function main() {
    let richEthWallet = new RichEthWallet();
    await richEthWallet.prepare();

    let wallets = [];
    for (let i = 0; i < NUM_WALLETS; i++) {
        wallets.push( new LocalWallet( 0 + i) );
    }

    console.log(" ##### PREPARING ##### ")
    await Promise.all(wallets.map(w => w.prepare(richEthWallet)));
    console.log(" ##### SENDING SOME ##### ")
    await Promise.all(wallets.map(w => richEthWallet.sendSome(w.wallet.ethWallet, "1000000000")));
    wallets.forEach(w => {
        console.log(`${w.wallet.address} contract: ${w.wallet.ethState.contractBalances[0]}`);
    }); 
    console.log(" ##### DEPOSITING ##### ")
    await Promise.all(wallets.map(w => w.deposit()));
    await Promise.all(wallets.map(w => w.depositOffchain()));

    wallets.forEach(w => {
        console.log(`${w.wallet.address} contract: ${w.wallet.ethState.contractBalances[0]}`);
    }); 
    wallets.forEach(w => {
        let com = w.wallet.franklinState.commited.balances[0];
        let verif = w.wallet.franklinState.verified.balances[0];
        let diff = new BN(com, 10).sub(new BN(verif, 10)).toString(10);
        console.log(`${w.wallet.address} com: ${w.wallet.franklinState.commited.balances[0]}, d verif: ${diff}`);
    }); 

    return;

    const prando = new Prando();
    for (let i = 0; i < 10; i++) {
        console.log(`loop iteration ${i}:`);
        await Promise.all(wallets.map(async w => {
            let w2 = prando.nextArrayItem(wallets);
            if (w === w2) return;

            await Promise.all([
                w.wallet.waitPendingTxsExecuted(),
                w2.wallet.waitPendingTxsExecuted()
            ]);
            
            let address = w2.wallet.address;

            let token = w.wallet.supportedTokens[0];

            let amount = new BN("1");
            let have = new BN(w.wallet.franklinState.verified.balances[token.id] || "0");
            amount = amount.lt(have) ? amount : have;

            let fee = new BN("0");

            let res = await w.wallet.transfer(address, token, amount, fee);
            console.log(`${w.wallet.address} sent ${res.hash}`);
            if (res.err !== null) {
                console.log(`err: ${res.err}`);
            }
        }));
    }

    console.log("WAITING PENDING");
    await Promise.all(wallets.map(w => w.wallet.waitPendingTxsExecuted()));
    console.log("WAITING PROVING");
    await Promise.all(wallets.map(w => w.wallet.waitProved()));

    wallets.forEach(w => {
        let com = w.wallet.franklinState.commited.balances[0];
        let verif = w.wallet.franklinState.verified.balances[0];
        let diff = new BN(com, 10).sub(new BN(verif, 10)).toString(10);
        console.log(`${w.wallet.address} com: ${w.wallet.franklinState.commited.balances[0]}, d verif: ${diff}`);
    }); 

    
    // const prando = new Prando();
    // let transactions = wallets.map(async w1 => {
    //     let w2 = prando.nextArrayItem(wallets);
    //     w1.wallet.
    // });



    // const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    // let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    // let wallet = await Wallet.fromEthWallet(ethWallet);
    // let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
    // let wallet2 = await Wallet.fromEthWallet(ethWallet2);

    // await wallet.updateState();
    // await wallet2.updateState();


    // // console.log(await wallet.depositOffchain(wallet.supportedTokens['0'], new BN(18), new BN(2)));
    // // await wallet.updateState();
    // // console.log(wallet.franklinState);

    // console.log(await wallet.depositOnchain(wallet.supportedTokens['0'], bigNumberify(20)));
    // await sleep(5000);
    // console.log(await wallet.depositOffchain(wallet.supportedTokens['0'], new BN(18), new BN(2)));
    // await wallet.waitPendingTxsExecuted();
    // console.log(await wallet.transfer(wallet2.address, wallet.supportedTokens['0'], new BN(15), new BN(3)));
    // await wallet.waitPendingTxsExecuted();
    // console.log(await wallet2.widthdrawOffchain(wallet2.supportedTokens['0'],new BN(10), new BN(5)));
    // // console.log(await wallet2.widthdrawOnchain(wallet2.supportedTokens['0'],bigNumberify(1));
    // await wallet.waitPendingTxsExecuted();
    
    // await wallet2.updateState();
    // console.log("offchain 2", wallet2.franklinState);
    // console.log("onchain 2", wallet2.ethState);
}

main();
