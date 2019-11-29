/**
 * Here we want to test a very specific scenario:
 * create 3 wallets, have the first receive money, then deposit, then transfer some on another franklin account.
 */

import { Tester } from '../Tester';
import fs from 'fs';
import { bigNumberify } from 'ethers/utils';

async function test(): Promise<void> {
    const tester: Tester = await Tester.new({
        initNumWallets: 3,
        randomSeed: 'whateverstring'
    });

    const exit = async () => {
        let path = '../../logs/loadtestlogs.json';
        console.log(`saving result to ${path} and exiting`);
        fs.writeFileSync(path, await tester.dump());
        process.exit(0);
    }

    process.once('SIGINT', exit);

    tester.addOperation(tester.randomReceiveMoneyOperation({
        wallet: tester.wallets[0],
        token: tester.tokens[0],
        amount: bigNumberify('1' + '0'.repeat(15))
    }));
    tester.addOperation(tester.randomDepositOperation({
        wallet: tester.wallets[0], 
        token: tester.tokens[0], 
        amount: bigNumberify('100000')
    }));
    tester.addOperation(tester.randomTransferOperation({
        wallet1: tester.wallets[0],
        wallet2: tester.wallets[1],
        token: tester.tokens[0],
        amount: bigNumberify('10000'),
        fee: bigNumberify('10')
    }));

    await tester.run();

    exit();
}

test()
