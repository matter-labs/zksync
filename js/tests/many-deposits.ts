const ethers = require('ethers');
import * as zksync from 'zksync';

import * as assert from 'assert';
import * as utils from './utils';
import { WalletDecorator, /* tokens */ } from './WalletDecorator';
const cliProgress = require('cli-progress');

const NUM_WALLETS    = 64;
const DEPOSIT_AMOUNT = ethers.utils.parseEther('0.1');

assert(utils.isPowerOfTwo(NUM_WALLETS));

async function test() {
    await WalletDecorator.waitReady();
    const tokens = ['ETH'];

    if (await WalletDecorator.isExodus()) {
        console.log(`📕 it's Exodus.`);
    }

    const wallet = await WalletDecorator.fromId(0);
    await wallet.prettyPrintBalances(tokens);

    const numDeposits = 800;
    const promises = [];

    const multibar = new cliProgress.MultiBar({
        clearOnComplete: false,
        hideCursor: true
    }, cliProgress.Presets.shades_grey);
    const sentProgressBar    = multibar.create(numDeposits, 0);
    const receiptProgressBar = multibar.create(numDeposits, 0);

    for (const i of utils.range(numDeposits)) {
        if (i % 10 === 0) {
            if (await WalletDecorator.isExodus()) {
                break;
            }
        }

        const receiptUpdater = i => () => receiptProgressBar.update(i);
        const promise = wallet
            .deposit(DEPOSIT_AMOUNT.div(numDeposits), tokens)
            .then(receiptUpdater(i + 1));
        
        promises.push(promise);
        sentProgressBar.update(i + 1);
    }

    await Promise.race([
        Promise.all(promises),
        WalletDecorator.waitExodus('print'),
    ]);

    // await WalletDecorator.waitExodus();

    await wallet.prettyPrintBalances(tokens);
}

test();
