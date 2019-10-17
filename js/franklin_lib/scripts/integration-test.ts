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

    let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
    let wallet2 = await Wallet.fromEthWallet(ethWallet2);

    wallet.provider.getAccountUpdates(wallet.address, "commit").onmessage = (evt => console.log("event: ", evt.data) );

    // let onchainBalances = await wallet2.getOnchainBalances();
    // onchainBalances.contractBalances.map((value => console.log(value.toString())));

    // fund wallet 2
    await ethWallet.sendTransaction({to: ethWallet2.address, value: ethers.utils.parseEther("1")});

    let depHandle = await wallet.deposit("ETH", ethers.utils.parseEther("0.1"), ethers.utils.parseEther("0.01"));
    await depHandle.waitCommit();
    console.log("Deposit commited");

    let transferHandle = await wallet.transfer(wallet2.address, "ETH", ethers.utils.parseEther("0.1"), 0);
    await transferHandle.waitCommit();
    console.log("Transfer commited");

    let withdrawOffchainHandle = await wallet2.widthdrawOffchain("ETH", ethers.utils.parseEther("0.1"), 0);
    await withdrawOffchainHandle.waitVerify();
    console.log("Withdraw verified");

    let onchainWithdrawHandle = await wallet2.widthdrawOnchain("ETH", ethers.utils.parseEther("0.1"));
    await onchainWithdrawHandle.wait();
    console.log(`Onchain withdraw successful ${onchainWithdrawHandle.hash}`);
}

main();
