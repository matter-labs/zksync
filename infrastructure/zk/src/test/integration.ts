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
    const server = utils.background(
        'cargo run --bin zksync_server --release',
        [0, serverLog, serverLog] // redirect stdout and stderr to server.log
    );
    await utils.sleep(1);

    const proverLog = fs.openSync('dummy_prover.log', 'w');
    const prover = utils.background(
        'cargo run --bin dummy_prover --release dummy-prover-instance',
        [0, proverLog, proverLog] // redirect stdout and stderr to dummy_prover.log
    );
    await utils.sleep(10);

    // set a timeout in case tests hang
    const timer = setTimeout(() => {
        console.log('Timeout reached!');
        process.exit(1);
    }, timeout * 1000);
    timer.unref(); // this is here to make sure process does not wait for timeout to fire

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

    // this code runs when tests finish or fail,
    // ctrl+c is pressed an or external kill signal is received
    process.on('exit', (code) => {
        console.log('Termination started...');
        // sleeps are here to let every process finish its work
        utils.sleepSync(5);
        // server or prover might also crash, so killing may be unsuccessful
        // but we still want to see the logs in this case.
        // using process.kill(-child.pid) and not child.kill() along with the fact than
        // child is detached guarantees that processes are killed recursively (by group id)
        utils.allowFailSync(() => process.kill(-server.pid, 'SIGKILL'));
        utils.allowFailSync(() => process.kill(-prover.pid, 'SIGKILL'));
        utils.allowFailSync(() => clearTimeout(timer));
        if (code !== 0) {
            // we only wan't to see the logs if something's wrong
            run.catLogs();
        }
        utils.sleepSync(5);
    });

    await testSuite();
    // without this, process hangs even though timeout is .unref()'d and tests finished sucessfully
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
        `docker run -v ${volume} ${image} bash -c "/usr/local/bin/entrypoint.sh && ${command} || zk run cat-logs 1"`
    );
}

export async function all() {
    await server();
    await api();
    await withdrawalHelpers();
    await zcli();
    await rustSDK();
    // have to kill server before running data-restore
    await utils.spawn('killall zksync_server');
    await run.dataRestore.checkExisting();
}

export async function api() {
    await utils.spawn('yarn ts-tests api-test');
}

export async function zcli() {
    await utils.spawn('yarn zcli test');
}

export async function server() {
    await utils.spawn('yarn ts-tests test');
}

export async function withdrawalHelpers() {
    await utils.spawn('yarn ts-tests withdrawal-helpers-test');
}

export async function testkit(command: string, timeout: number) {
    let containerID = '';
    const prevUrls = process.env.ETH_CLIENT_WEB3_URL?.split(',')[0];
    if (process.env.ZKSYNC_ENV == 'dev' && process.env.CI != '1') {
        const { stdout } = await utils.exec('docker run --rm -d -p 7545:8545 matterlabs/geth:latest fast');
        containerID = stdout;
        process.env.ETH_CLIENT_WEB3_URL = 'http://localhost:7545';
    }
    process.on('SIGINT', () => {
        console.log('interrupt received');
        // we have to emit this manually, as SIGINT is considered explicit termination
        process.emit('beforeExit', 130);
    });

    // set a timeout in case tests hang
    const timer = setTimeout(() => {
        console.log('Timeout reached!');
        process.emit('beforeExit', 1);
    }, timeout * 1000);
    timer.unref();

    // since we HAVE to make an async call upon exit,
    // the only solution is to use beforeExit hook
    // but be careful! this is not called upon explicit termination
    // e.g. on SIGINT or process.exit()
    process.on('beforeExit', async (code) => {
        if (process.env.ZKSYNC_ENV == 'dev' && process.env.CI != '1') {
            try {
                // probably should be replaced with child_process.execSync in future
                // to change the hook to program.on('exit', ...)
                await utils.exec(`docker kill ${containerID}`);
            } catch {
                console.error('Problem killing', containerID);
            }
            process.env.ETH_CLIENT_WEB3_URL = prevUrls;
            // this has to be here - or else we will call this hook again
            process.exit(code);
        }
    });

    process.env.CHAIN_ETH_NETWORK = 'test';
    await run.verifyKeys.unpack();
    await contract.build();

    if (command.includes('block_sizes_test ')) {
        await utils.spawn(`cargo run --release --bin ${command}`);
    } else if (command == 'fast') {
        // await utils.spawn('cargo run --bin testkit_tests --release');
        await utils.spawn('cargo run --bin gas_price_test --release');
        // await utils.spawn('cargo run --bin revert_blocks_test --release');
        // await utils.spawn('cargo run --bin migration_test --release');
        // await utils.spawn('cargo run --bin exodus_test --release');
    } else {
        await utils.spawn(`cargo run --bin ${command} --release`);
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
    .command('withdrawal-helpers')
    .description('run withdrawal helpers integration tests')
    .option('--with-server')
    .action(async (cmd: Command) => {
        cmd.withServer ? await withServer(withdrawalHelpers, 1200) : await withdrawalHelpers();
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
    .option('--offline')
    .action(async (mode?: string, offline: boolean = false) => {
        if (offline) {
            process.env.SQLX_OFFLINE = 'true';
        }
        mode = mode || 'fast';
        await testkit(mode, 6000);

        if (offline) {
            delete process.env.SQLX_OFFLINE;
        }
    });
