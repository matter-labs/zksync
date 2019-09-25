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
    // TODO: unimpl use new deposits, test full exit.
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    let wallet = await Wallet.fromEthWallet(ethWallet);
    await wallet.updateState();
    // let full_exit_tx = await wallet.emergencyWithdraw({id: 0, address: ethers.constants.AddressZero});
    // console.log(full_exit_tx);
    let dep_tx = await wallet.deposit(wallet.supportedTokens['1'], 10);
    console.log(dep_tx);

    // await wallet.updateState();
    // console.log(wallet.supportedTokens);
    // console.log(wallet.franklinState);
    // console.log(wallet.ethState);
    // let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
    // let wallet2 = await Wallet.fromEthWallet(ethWallet2);
    //
    // await wallet.updateState();
    // await wallet2.updateState();


    // console.log(await wallet.transfer(wallet2.address, wallet.supportedTokens['0'], parseEther("1"),parseEther("0.1")));
    // await wallet.waitPendingTxsExecuted();
    // console.log(await wallet.transfer(wallet2.address, wallet.supportedTokens['1'], 15,3));
    // await wallet.waitPendingTxsExecuted();

    // console.log(await wallet2.widthdrawOffchain(wallet2.supportedTokens['0'], 9, 0));
    // await wallet2.waitPendingTxsExecuted();
    // console.log(await wallet2.widthdrawOffchain(wallet2.supportedTokens['1'], 10, 5));
    // await wallet2.waitPendingTxsExecuted();
    //
    // await wallet2.updateState();
    // console.log("offchain 2", wallet2.franklinState);
    // console.log("onchain 2", wallet2.ethState);
}

main();
