/**
 * Here we create a lot of wallets,
 * assign a lot of totally random operations to them,
 * and watch the server struggle.
 */
import { Tester } from '../Tester';
import fs, { exists } from 'fs';
async function test(): Promise<void> {
    const tester: Tester = await Tester.new({
        initNumWallets: 10,
        randomSeed: 'whateverstring',
    });

    const exit = () => {
        let path = '../../logs/loadtestlogs.json';
        console.log(`saving result to ${path} and exiting`);
        fs.writeFileSync(path, tester.dump());
        process.exit(0);
    };

    process.once('SIGINT', exit);

    for (let i = 0; i < 500; i++) {
        tester.addOperation(null);
    }

    await tester.run();

    exit();
}

test();
