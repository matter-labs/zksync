const ethers = require('ethers');
import * as zksync from 'zksync';

import * as assert from 'assert';
import * as utils from './utils';
import { WalletDecorator, /* tokens */ } from './WalletDecorator';

const NUM_WALLETS    = 32;
const DEPOSIT_AMOUNT = ethers.utils.parseEther('10');

assert(utils.isPowerOfTwo(NUM_WALLETS));

async function test() {
    await WalletDecorator.waitReady();
    const tokens = ['ETH'];

    const [richWallet, ...wallets] = await Promise.all(utils.rangearr(NUM_WALLETS).map(WalletDecorator.fromId));
    await richWallet.deposit(DEPOSIT_AMOUNT, tokens);
    await richWallet.setCurrentPubkeyWithZksyncTx();
    await richWallet.prettyPrintBalances(tokens);

    await Promise.all(
        wallets.map(
            wallet => richWallet.transfer(wallet, DEPOSIT_AMOUNT.div(100), tokens)
        )
    );

    await Promise.all(
        wallets.slice(4).map(
            wallet => richWallet.transfer(wallet, DEPOSIT_AMOUNT.div(100), tokens)
        )
    );
}

test();
