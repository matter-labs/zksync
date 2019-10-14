import BN = require('bn.js');
import { Wallet } from '../src/wallet';
import { ethers } from 'ethers';
import {bigNumberify, parseEther, formatEther} from "ethers/utils";

const WALLETS=100;

function sleep(ms) {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}

async function makeTxsWallet(wallet: Wallet, wallets: Wallet[]) {
    let nonce = await wallet.getNonce("commited");
    for (let i = 0; i < WALLETS*5; ++i) {
        let target = wallets[i % wallets.length];
        if (target.address == wallet.address) {
            continue;
        }

        let amount = parseEther("0.0001");
        let transferHandle = await wallet.transfer(target.address, 0, amount, 0, nonce++);
        console.log(`${wallet.address.toString("hex")} -> ${target.address.toString("hex")} amount: ${formatEther(amount)} eth , nonce: ${nonce}` );
        await transferHandle.waitCommit();
    }
}

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

    // main wallet with money
    let mainEthWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    let mainFranklinWallet = await Wallet.fromEthWallet(mainEthWallet);
    // let deposit = await mainFranklinWallet.deposit(0, parseEther((1*WALLETS).toString()), parseEther("0.1"));
    // await deposit.waitCommit();


    // init test wallets.
    let wallets = [];
    let lastFundTx;
    let nonce = await mainFranklinWallet.getNonce("commited");
    for(let i = 0; i < WALLETS; ++i) {
        let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, `m/44'/60'/0'/0/${i + 2}`).connect(provider);
        let franklinWallet = await Wallet.fromEthWallet(ethWallet);
        lastFundTx = await mainFranklinWallet.transfer(franklinWallet.address, 0, parseEther("1"), 0, nonce++);
        wallets.push(franklinWallet);
    }
    await lastFundTx.waitCommit();

    for(let i = 0; i < WALLETS; ++i) {
        makeTxsWallet(wallets[i], wallets).catch(( e => console.log(`Wallet ${i} error: ${e.toString()}`)));
    }

}

main();
