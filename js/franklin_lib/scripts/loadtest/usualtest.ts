import { Tester } from './Tester';
import fs, { exists } from 'fs';
import { bigNumberify } from 'ethers/utils';
async function test(): Promise<void> {
    const tester: Tester = await Tester.new(50);

    const exit = async () => {
        let path = '/Users/oleg/Desktop/loadtestlogs.json';
        console.log(`saving result to ${path} and exiting`);
        fs.writeFileSync(path, await tester.dump());
        process.exit(0);
    }

    process.once('SIGINT', exit);


    tester.wallets.forEach(w => {
        tester.addOperation(tester.randomReceiveMoneyOperation({wallet: w, token: tester.tokens[0]}));
        tester.addOperation(tester.randomReceiveMoneyOperation({wallet: w, token: tester.tokens[1]}));
        tester.addOperation(tester.randomDepositOperation({wallet: w, token: tester.tokens[0], amount: bigNumberify('100000')}))
        tester.addOperation(tester.randomDepositOperation({wallet: w, token: tester.tokens[1], amount: bigNumberify('100000')}))
        // tester.addOperation(tester.randomWithdrawOperation({wallet: w, token: tester.tokens[0], amount: bigNumberify('50000')}))
        // tester.addOperation(tester.randomWithdrawOperation({wallet: w, token: tester.tokens[1], amount: bigNumberify('50000')}))
    });

    for (let i = 0; i < 200; i++) {
        tester.addOperation(tester.randomTransferOperation({}));
    }

    await tester.run();

    exit();
}

test()
