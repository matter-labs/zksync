import { Tester } from '../Tester';
import fs, { exists } from 'fs';
import { parseEther, bigNumberify, BigNumber, BigNumberish } from 'ethers/utils';

async function test(): Promise<void> {
    let initNumWallets    = Number(process.argv[2]);
    let shardWalletOffset = Number(process.argv[3]);
    let shardWalletLimit  = Number(process.argv[4]);
    let runGoal           = String(process.argv[5]);

    const tester = await Tester.new({
        initNumWallets,
        shardWalletOffset,
        shardWalletLimit,
        randomSeed: 'whateverstring'
    });

    const exit = async () => {
        let path = `../../logs/loadtestlogs${shardWalletOffset / shardWalletLimit}.json`;
        console.log(`saving result to ${path} and exiting`);
        fs.writeFileSync(path, await tester.dump());
        process.exit(0);
    }
    process.once('SIGINT', exit);

    if (runGoal == 'replenish') {
        tester.wallets.forEach(wallet => {
            tester.addOperation(tester.randomReceiveMoneyOperation({wallet, token: tester.tokens[0], amount: parseEther('1000.5') }));
            tester.addOperation(tester.randomReceiveMoneyOperation({wallet, token: tester.tokens[1], amount: parseEther('1000.5') }));
        });
        tester.runForAll();
    } else {
        tester.walletsShard.forEach(wallet => {
            tester.addOperation(tester.randomDepositOperation({wallet, token: tester.tokens[0], amount: parseEther('1000'), fee: parseEther('0.01')}));
            tester.addOperation(tester.randomDepositOperation({wallet, token: tester.tokens[1], amount: parseEther('1000'), fee: parseEther('0.1')}));
        });
    
        for (let i = 0; i < 10; i++) {
            tester.addOperation(tester.randomTransferOperation({}));
        }
    
        // tester.walletsShard.forEach(wallet => {
        //     tester.addOperation(tester.randomWithdrawOperation({wallet, token: tester.tokens[0], amount: bigNumberify('100')}));
        // });
    
        await tester.run();
    
        exit();
    }
}

test().catch(e => {
    console.log('got error', e.message);
    process.exit(1);
})
