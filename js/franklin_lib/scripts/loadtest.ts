import BN = require('bn.js');
import { Wallet } from '../src/wallet';
import { ethers } from 'ethers';
import {bigNumberify} from "ethers/utils";

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
    console.log(await wallet.depositOnchain(wallet.supportedTokens['1'], bigNumberify(10)));
    sleep(1000);
    console.log(await wallet.depositOffchain(wallet.supportedTokens['1'], new BN(9), new BN(1)));

    sleep(10000);

    let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
    let wallet2 = await Wallet.fromEthWallet(ethWallet2);

    console.log(await wallet.transfer(wallet2.address, wallet.supportedTokens['1'], new BN(2), new BN(0)));

    sleep(5000);
    await wallet.getState();
    await wallet2.getState();
    console.log(wallet2.franklinState);
    console.log(wallet.franklinState);
}

main();
