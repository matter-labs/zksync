import BN = require('bn.js');
import { Wallet } from '../src/wallet';
import { ethers } from 'ethers';

function sleep(ms) {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}

async function main() {
    let provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
    let richWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC).connect(provider);
    let richFranklinWallet = await Wallet.fromEthWallet(richWallet);    
    console.log('richFranklinWallet address: ', richFranklinWallet.address);

    let ethWallet1 = ethers.Wallet.createRandom().connect(provider);
    let ethWallet2 = ethers.Wallet.createRandom().connect(provider);
    let ethWallet3 = ethers.Wallet.createRandom().connect(provider);

    let acc1 = await Wallet.fromEthWallet(ethWallet1);
    let acc2 = await Wallet.fromEthWallet(ethWallet2);
    let acc3 = await Wallet.fromEthWallet(ethWallet3);
    
    await richFranklinWallet.deposit(0, new BN('1000'), new BN('1'));
    console.log('after deposit: ', await richFranklinWallet.ethWallet.getBalance());

    console.log('getstate:', await richFranklinWallet.provider.getState(richFranklinWallet.address));

    // console.log('Before deposit:');
    // console.log('Account 1', await acc1.getState());
    // console.log('Account 2', await acc2.getState());
    // console.log('Account 3', await acc3.getState());

    // let Account1Address = Buffer.from('dead00000000000000000000000000000000000000000000000000', 'hex');
    // let Account2Address = Buffer.from('beef00000000000000000000000000000000000000000000000000', 'hex');
    // let Account3Address = Buffer.from('babe00000000000000000000000000000000000000000000000000', 'hex');
    // let feeCollectorAddress = Buffer.from('000000000000000000000000000000000000000000000000000000', 'hex');
    // let defaultFranklinProvider = new FranklinProvider();
    // let acc1 = new Wallet(Account1Address, defaultFranklinProvider);
    // let acc2 = new Wallet(Account2Address, defaultFranklinProvider);
    // let acc3 = new Wallet(Account3Address, defaultFranklinProvider);
    // let feeAccount = new Wallet(feeCollectorAddress, defaultFranklinProvider);

    // console.log('Before deposit');
    // console.log('Account 1', await acc1.getState());
    // console.log('Account 2', await acc2.getState());
    // console.log('Account 3', await acc3.getState());
    // console.log('FeeAccount', await feeAccount.getState());

    // await acc1.deposit(0, new BN(100 + 45 + 8), new BN(10));
    // await acc2.deposit(0, new BN(100), new BN(12));

    // await sleep(10000);
    // console.log('After deposit');
    // console.log('Account 1', await acc1.getState());
    // console.log('Account 2', await acc2.getState());
    // console.log('Account 3', await acc3.getState());
    // console.log('FeeAccount', await feeAccount.getState());

    // await acc1.transfer(acc2.address, 0, new BN(45), new BN(8));
    // await acc2.transfer(acc3.address, 0, new BN(90), new BN(10));

    // await sleep(10000);
    // console.log('After Transfer');
    // console.log('Account 1', await acc1.getState());
    // console.log('Account 2', await acc2.getState());
    // console.log('Account 3', await acc3.getState());
    // console.log('FeeAccount', await feeAccount.getState());
}

main();
