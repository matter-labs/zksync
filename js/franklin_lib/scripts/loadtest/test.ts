import { Tester } from './Tester';
import fs, { exists } from 'fs';
async function test(): Promise<void> {
    const tester: Tester = await Tester.new(100);

    const exit = () => {
        let path = '/Users/oleg/Desktop/loadtestlogs.json';
        console.log(`saving result to ${path} and exiting`);
        fs.writeFileSync(path, tester.dump());
        process.exit(0);
    }

    process.once('SIGINT', exit);

    for (let i = 0; i < 500; i++) {
        tester.addOperation(null);
    }

    await tester.run();

    exit();
}

test()
