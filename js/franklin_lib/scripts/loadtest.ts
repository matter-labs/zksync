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
    // let provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
    // let richWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC).connect(provider);
    // let richFranklinWallet = await Wallet.fromEthWallet(richWallet);    
    // console.log('richFranklinWallet address: ', richFranklinWallet.address);

    // let ethWallet1 = ethers.Wallet.createRandom().connect(provider);
    // let ethWallet2 = ethers.Wallet.createRandom().connect(provider);
    // let ethWallet3 = ethers.Wallet.createRandom().connect(provider);

    // let acc1 = await Wallet.fromEthWallet(ethWallet1);
    // let acc2 = await Wallet.fromEthWallet(ethWallet2);
    // let acc3 = await Wallet.fromEthWallet(ethWallet3);
    
    // await richFranklinWallet.deposit(0, new BN('1000'), new BN('1'));
    // console.log('after deposit: ', await richFranklinWallet.ethWallet.getBalance());

    // console.log('getstate:', await richFranklinWallet.getState());

    // console.log('getverifiedstate:', await richFranklinWallet.getVerifiedFranklinState());

    // console.log(richFranklinWallet.contract);

    // console.log(await richFranklinWallet.getCommittedOnchainState());

    // console.log(await richFranklinWallet.contract.getMyBalanceForToken(1));
    // console.log(await richFranklinWallet.contract.getMyBalanceForTokenAndAddress(richFranklinWallet.ethWallet.address, 1));
    // console.log(richFranklinWallet.ethWallet.address);
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC).connect(provider);
    let wallet = await Wallet.fromEthWallet(ethWallet);

    await wallet.getState();
    console.log('onchain balance:', await wallet.getCommittedOnchainState());
    console.log("await wallet.depositOnchain(wallet.supportedTokens['1'], bigNumberify(10))", 
                 await wallet.depositOnchain(wallet.supportedTokens['1'], bigNumberify(10)));
                 
    await sleep(1000);
    console.log("await wallet.depositOffchain(wallet.supportedTokens['1'], new BN(9), new BN(1))", 
                 await wallet.depositOffchain(wallet.supportedTokens['1'], new BN(8), new BN(1)));

    await sleep(10000);

    let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
    let wallet2 = await Wallet.fromEthWallet(ethWallet2);

    console.log("await wallet.transfer(wallet2.address, wallet.supportedTokens['1'], new BN(2), new BN(0))", await wallet.transfer(wallet2.address, wallet.supportedTokens['1'], new BN(2), new BN(0)));

    await sleep(5000);
    await wallet.getState();
    await wallet2.getState();
    console.log("wallet2.franklinState", wallet2.franklinState);
    console.log("wallet.franklinState", wallet.franklinState);
}

main();
