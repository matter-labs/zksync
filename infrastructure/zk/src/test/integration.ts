import { Command } from 'commander';
import * as utils from '../utils';
import fs from 'fs';
import * as dummyProver from '../dummy-prover';
import * as contract from '../contract';

export async function withServer(testSuite: CallableFunction, timeout: number) {
	if (!await dummyProver.status()) {
		await dummyProver.enable();
	}

	await utils.spawn('cargo build --bin zksync_server --release');
	await utils.spawn('cargo build --bin dummy_prover --release');

	const server = utils.background('cargo run --bin zksync_server --release &> server.log');
	await utils.sleep(1);

	const prover = utils.background('cargo run --bin dummy_prover --release dummy-prover-instance &> prover.log');
	await utils.sleep(10)

    const timer = setTimeout(() => {
        console.log('Timeout reached!');
        process.exit(1);
    }, timeout * 1000);
    timer.unref();

    process.on('SIGINT', () => {
        console.log('Interrupt received...')
        process.exit(130);
    });
    
    process.on('SIGTERM', () => {
        console.log('Being murdered...')
        process.exit(143);
    });

    process.on('exit', (code) => {
        console.log('Termination started...');
        utils.sleepSync(5);
        utils.allowFailSync(() => process.kill(-server.pid, 'SIGKILL'));
        utils.allowFailSync(() => process.kill(-prover.pid, 'SIGKILL'));
        utils.allowFailSync(() => clearTimeout(timer));
        console.log('\nSERVER LOGS:\n', fs.readFileSync('server.log').toString());
        console.log('\nPROVER LOGS:\n', fs.readFileSync('prover.log').toString());
        utils.sleepSync(5);
        console.log('Exit code:', code);
	});

    await testSuite();
}

export async function api() {
	await utils.spawn('yarn --cwd core/tests/ts-test api-test');
}

export async function zcli() {
	await utils.spawn('yarn --cwd infrastructure/zcli test');
}

export async function server() {
	await utils.spawn('yarn --cwd core/tests/ts-test test');
}

export async function testkit(command: string) {
	let containerID = '';
	const prevUrl = process.env.WEB3_URL;
	if (process.env.ZKSYNC_ENV == 'ci') {
		process.env.WEB3_URL = 'http://geth-fast:8545'
	} else if (process.env.ZKSYNC_ENV == 'dev') {
		let { stdout } = await utils.exec('docker run --rm -d -p 7545:8545 matterlabs/geth:latest fast');
		containerID = stdout;
		process.env.WEB3_URL = 'http://localhost:7545';
	}
	process.env.ETH_NETWORK = 'test';
	await contract.build();

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

export const command = new Command('integration')
	.description('zksync integration tests')
	.alias('i');

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
