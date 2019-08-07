import BN = require('bn.js');
import { Wallet } from '../src/wallet';
import { ethers } from 'ethers';

function sleep(ms) {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}

async function main() {
    let Account1ETH = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2");
    let Account2ETH = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/3");
    let Account3ETH = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/4");
    let feeCollector = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1");
    let acc1 = await Wallet.fromEthWallet(Account1ETH);
    let acc2 = await Wallet.fromEthWallet(Account2ETH);
    let acc3 = await Wallet.fromEthWallet(Account3ETH);
    let feeAccount = await Wallet.fromEthWallet(feeCollector);
    console.log(feeAccount.address);
    console.log(acc1.address);

    console.log('Before deposit');
    console.log('Account 1', await acc1.getState());
    console.log('Account 2', await acc2.getState());
    console.log('Account 3', await acc3.getState());
    console.log('FeeAccount', await feeAccount.getState());

    await acc1.deposit(0, new BN(100 + 45 + 8), new BN(10));
    await acc2.deposit(0, new BN(100), new BN(12));

    await sleep(10000);
    console.log('After deposit');
    console.log('Account 1', await acc1.getState());
    console.log('Account 2', await acc2.getState());
    console.log('Account 3', await acc3.getState());
    console.log('FeeAccount', await feeAccount.getState());

    await acc1.transfer(acc2.address, 0, new BN(45), new BN(8));
    await acc2.transfer(acc3.address, 0, new BN(90), new BN(10));

    await sleep(10000);
    console.log('After Transfer');
    console.log('Account 1', await acc1.getState());
    console.log('Account 2', await acc2.getState());
    console.log('Account 3', await acc3.getState());
    console.log('FeeAccount', await feeAccount.getState());
}

main();
