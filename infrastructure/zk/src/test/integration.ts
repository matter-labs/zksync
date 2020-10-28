import { Command } from 'commander';
import * as utils from '../utils';
import fs from 'fs';
import * as dummyProver from '../dummy-prover';
import * as contract from '../contract';
import * as run from '../run/run';

export async function withServer(testSuite: () => Promise<void>, timeout: number) {
    if (!(await dummyProver.status())) {
        await dummyProver.enable();
    }

    await utils.spawn('cargo build --bin zksync_server --release');
    await utils.spawn('cargo build --bin dummy_prover --release');

    const serverLog = fs.openSync('server.log', 'w');
    const server = utils.background('cargo run --bin zksync_server --release', [0, serverLog, serverLog]);
    await utils.sleep(1);

    const proverLog = fs.openSync('dummy_prover.log', 'w');
    // prettier-ignore
    const prover = utils.background(
        'cargo run --bin dummy_prover --release dummy-prover-instance',
        [0, proverLog, proverLog]
    );
    await utils.sleep(10);

    const timer = setTimeout(() => {
        console.log('Timeout reached!');
        process.exit(1);
    }, timeout * 1000);
    timer.unref();

    // for unknown reason, when ctrl+c is pressed, the exit hook
    // is only triggered after the current process has exited,
    // so you will see logs appearing out of nowhere.
    // child processes are detached (because it's the only
    // way to kill them recursively in node), but the hook itself
    // is not, despite its weid behaviour. if anyone can fix this,
    // I would appreciate it, otherwise, it's not a big deal,
    // since --with-server is only meant to be run on CI.
    process.on('SIGINT', () => {
        console.log('Interrupt received...');
        process.exit(130);
    });

    process.on('SIGTERM', () => {
        console.log('Being murdered...');
        process.exit(143);
    });

    process.on('exit', (code) => {
        console.log('Termination started...');
        utils.sleepSync(5);
        utils.allowFailSync(() => process.kill(-server.pid, 'SIGKILL'));
        utils.allowFailSync(() => process.kill(-prover.pid, 'SIGKILL'));
        utils.allowFailSync(() => clearTimeout(timer));
        if (code !== 0) {
            run.catLogs();
        }
        utils.sleepSync(5);
    });

    await testSuite();
    process.exit(0);
}

export async function inDocker(command: string, timeout: number) {
    const timer = setTimeout(() => {
        console.log('Timeout reached!');
        run.catLogs(1);
    }, timeout * 1000);
    timer.unref();

    const volume = `${process.env.ZKSYNC_HOME}:/usr/src/zksync`;
    const image = `matterlabs/ci-integration-test:latest`;
    await utils.spawn(
        `docker run -v ${volume} ${image} bash -c "/usr/local/bin/entrypoint.sh && ${command} || zk run cat-logs"`
    );
}

export async function all() {
    await server();
    await api();
    await zcli();
    await rustSDK();
    await utils.spawn('killall zksync_server');
    await run.dataRestore.checkExisting();
}

export async function api() {
    await utils.spawn('yarn --cwd core/tests/ts-tests api-test');
}

export async function zcli() {
    await utils.spawn('yarn --cwd infrastructure/zcli test');
}

export async function server() {
    await utils.spawn('yarn --cwd core/tests/ts-tests test');
}

export async function testkit(command: string) {
    let containerID = '';
    const prevUrl = process.env.WEB3_URL;
    if (process.env.ZKSYNC_ENV == 'ci') {
        process.env.WEB3_URL = 'http://geth-fast:8545';
    } else if (process.env.ZKSYNC_ENV == 'dev') {
        const { stdout } = await utils.exec('docker run --rm -d -p 7545:8545 matterlabs/geth:latest fast');
        containerID = stdout;
        process.env.WEB3_URL = 'http://localhost:7545';
    }
    process.on('SIGINT', () => {
        console.log('interrupt received');
        process.emit('beforeExit', 130);
    });

    process.on('beforeExit', async (code) => {
        if (process.env.ZKSYNC_ENV == 'dev') {
            try {
                await utils.exec(`docker kill ${containerID}`);
            } catch {
                console.error('Problem killing', containerID);
            }
            process.env.WEB3_URL = prevUrl;
            process.exit(code);
        }
    });

    process.env.ETH_NETWORK = 'test';
    await contract.build();

    if (command == 'block-sizes') {
        await utils.spawn('cargo run --bin block_sizes_test --release');
    } else if (command == 'fast') {
        await utils.spawn('cargo run --bin zksync_testkit --release');
        await utils.spawn('cargo run --bin gas_price_test --release');
        await utils.spawn('cargo run --bin migration_test --release');
        await utils.spawn('cargo run --bin revert_blocks_test --release');
        await utils.spawn('cargo run --bin exodus_test --release');
    }
}

export async function rustSDK() {
    await utils.spawn('cargo test -p zksync --release -- --ignored --test-threads=1');
}

export const command = new Command('integration').description('zksync integration tests').alias('i');

command
    .command('all')
    .description('run all integration tests (no testkit)')
    .option('--with-server')
    .option('--in-docker')
    .action(async (cmd: Command) => {
        const timeout = 1800;
        if (cmd.withServer) {
            await withServer(all, timeout);
        } else if (cmd.inDocker) {
            await inDocker('zk test i all', timeout);
        } else {
            await all();
        }
    });

command
    .command('zcli')
    .description('run zcli integration tests')
    .option('--with-server')
    .action(async (cmd: Command) => {
        cmd.withServer ? await withServer(zcli, 240) : await zcli();
    });

command
    .command('server')
    .description('run server integration tests')
    .option('--with-server')
    .action(async (cmd: Command) => {
        cmd.withServer ? await withServer(server, 1200) : await server();
    });

command
    .command('rust-sdk')
    .description('run rust SDK integration tests')
    .option('--with-server')
    .action(async (cmd: Command) => {
        cmd.withServer ? await withServer(rustSDK, 1200) : await rustSDK();
    });

command
    .command('api')
    .description('run api integration tests')
    .option('--with-server')
    .action(async (cmd: Command) => {
        cmd.withServer ? await withServer(api, 240) : await api();
    });

command
    .command('testkit [mode]')
    .description('run testkit tests')
    .action(async (mode?: string) => {
        mode = mode || 'fast';
        if (mode != 'fast' && mode != 'block-sizes') {
            throw new Error('modes are either "fast" or "block-sizes"');
        }
        await testkit(mode);
    });
