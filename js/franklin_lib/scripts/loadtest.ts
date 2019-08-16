import BN = require('bn.js');
import { Wallet } from '../src/wallet';
import { ethers } from 'ethers';
import {bigNumberify, parseEther} from "ethers/utils";

function sleep(ms) {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    let wallet = await Wallet.fromEthWallet(ethWallet);

    await wallet.getState();
    // console.log(await wallet.depositOnchain(wallet.supportedTokens['0'], bigNumberify(20)));
    // await sleep(20000);
    // console.log(await wallet.depositOffchain(wallet.supportedTokens['0'], new BN(18), new BN(2)));
    // await sleep(30000);

    let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
    let wallet2 = await Wallet.fromEthWallet(ethWallet2);
    await wallet.getState();
    await wallet2.getState();
    // console.log(await wallet2.widthdrawOffchain(wallet2.supportedTokens['0'], new BN(4), new BN(0)));
    // console.log(await wallet.transfer(wallet2.address, wallet.supportedTokens['0'], new BN(4), new BN(0)));
    // await sleep(5000);
    // await wallet.getState();
    let tx = await ethWallet.sendTransaction({to: ethWallet2.address, value: parseEther("2.0")});
    await tx.wait();

    console.log(await wallet2.widthdrawOnchain(wallet2.supportedTokens['0'],  bigNumberify(4)));


    // await wallet2.getState();
    // console.log(wallet2.franklinState);
    // console.log(wallet.franklinState);
}

main();
