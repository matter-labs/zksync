import BN = require('bn.js');
import { Wallet } from '../src/wallet';

function sleep(ms) {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}

async function main() {
    let Account1Address = Buffer.from('dead00000000000000000000000000000000000000000000000000', 'hex');
    let Account2Address = Buffer.from('beef00000000000000000000000000000000000000000000000000', 'hex');
    let Account3Address = Buffer.from('babe00000000000000000000000000000000000000000000000000', 'hex');
    let feeCollectorAddress = Buffer.from('000000000000000000000000000000000000000000000000000000', 'hex');
    let acc1 = new Wallet(Account1Address);
    let acc2 = new Wallet(Account2Address);
    let acc3 = new Wallet(Account3Address);
    let feeAccount = new Wallet(feeCollectorAddress);

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
