/**
 * Here we create a lot of wallets,
 * every 
 */
import { Tester } from '../Tester';
import fs, { exists } from 'fs';
import { parseEther, bigNumberify, BigNumber, BigNumberish } from 'ethers/utils';

async function test(): Promise<void> {
    const tester: Tester = await Tester.new({
        initNumWallets: 30,
        randomSeed: 'whateverstring'
    });

    const exit = async () => {
        let path = '../../logs/loadtestlogs.json';
        console.log(`saving result to ${path} and exiting`);
        fs.writeFileSync(path, await tester.dump());
        process.exit(0);
    }

    process.once('SIGINT', exit);

    tester.wallets.forEach(w => {
        // tester.addOperation(tester.randomReceiveMoneyOperation({ wallet: w, token: tester.tokens[0], amount: parseEther('10.5') }));
        // tester.addOperation(tester.randomReceiveMoneyOperation({wallet: w, token: tester.tokens[1], amount: bigNumberify('1000000')}));
        // tester.addOperation(tester.randomDepositOperation({wallet: w, token: tester.tokens[0], amount: parseEther('10'), fee: parseEther('0.01')}));
        // tester.addOperation(tester.randomDepositOperation({wallet: w, token: tester.tokens[1], amount: bigNumberify('1000000'), fee: parseEther('0.1')}));
        // tester.addOperation(tester.randomWithdrawOperation({walletr: w, token: tester.tokens[0], amount: bigNumberify('100000')}));
    });

    for (let i = 0; i < 10; i++) {
        tester.addOperation(tester.randomTransferOperation({}));
    }

    // tester.wallets.forEach(w => {
    //     tester.addOperation(tester.randomWithdrawOperation({wallet: w, token: tester.tokens[0], amount: bigNumberify('100')}));
    // });

    await tester.run();

    exit();
}

test()
