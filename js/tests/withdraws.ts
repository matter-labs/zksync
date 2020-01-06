const ethers = require('ethers');
import * as assert from 'assert';
import * as utils from './utils';
import { WalletDecorator, tokens } from './WalletDecorator';

const NUM_WALLETS    = 64;
const DEPOSIT_AMOUNT = ethers.utils.parseEther('0.1');

assert(utils.isPowerOfTwo(NUM_WALLETS));

async function test() {
    await WalletDecorator.waitReady();

    const ids = [...utils.range(NUM_WALLETS)];
    const wallets = await Promise.all(ids.map(WalletDecorator.fromId));    

    // rich wallet
    if (true) {
        await wallets[0].prettyPrintBalances(tokens);
        await wallets[0].deposit(DEPOSIT_AMOUNT.mul(3), tokens);
        await wallets[0].prettyPrintBalances(tokens);
    }


    // lots of withdraws for one walelt in one block work.
    if (true) {
        const NUM_WITHDRAWS = 10;
        const WITHDRAW_AMOUNT = DEPOSIT_AMOUNT.div(NUM_WITHDRAWS);

        await Promise.all(
            [...utils.range(NUM_WITHDRAWS)]
            .map(_ => wallets[0].withdraw(WITHDRAW_AMOUNT, tokens))
        );
        
        await wallets[0].prettyPrintBalances(tokens);
    }
    
    
    // lots of transfers from one walelt
    if (true) {
        const richWallet = wallets[0];
        const otherWallets = wallets.slice(1);
        const TRANSFER_AMOUNT = DEPOSIT_AMOUNT.div(NUM_WALLETS);

        await richWallet.prettyPrintBalances(tokens);

        await Promise.all(
            otherWallets.map(
                wallet => Promise.all([
                    richWallet.transfer(wallet, TRANSFER_AMOUNT, tokens),
                    wallet.withdraw(TRANSFER_AMOUNT.div(2), tokens),
                    wallet.transfer(richWallet, TRANSFER_AMOUNT.div(2), tokens),
                ])
            )
        );

        await richWallet.prettyPrintBalances(tokens);
    }
}

test();
