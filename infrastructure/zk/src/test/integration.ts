import { Command } from 'commander';
import * as utils from '../utils';
import fs from 'fs';
import * as dummyProver from '../dummy-prover';


export async function withServer(testSuite: CallableFunction, timeout = 120) {
	if (!await dummyProver.status()) {
		await dummyProver.enable();
	}

	await utils.spawn('cargo build --bin zksync_server --release');
	await utils.spawn('cargo build --bin dummy_prover --release');

	const server = utils.background('cargo run --bin zksync_server --release &> server.log');
	await utils.sleep(1);

	const prover = utils.background('cargo run --bin dummy_prover --release dummy-prover-instance &> prover.log');
	await utils.sleep(10)

	const cleanup = async () => {
		console.log('Termination started...')
		await utils.sleep(5);
		server.kill();
		prover.kill();
		clearTimeout(timer);
		console.log('SERVER LOGS:')
		console.log(fs.readFileSync('server.log').toString());
		console.log('\n\nPROVER LOGS:')
		console.log(fs.readFileSync('prover.log').toString());
		await utils.sleep(5);
		process.exit()
	}

	const timer = setTimeout(cleanup, timeout * 1000);

	process.on('exit', cleanup);
	process.on('SIGINT', cleanup);
	process.on('uncaughtException', cleanup);

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

export async function testkit() {
	// TODO
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
		cmd.withServer ? await withServer(zcli) : await zcli();
	});

command
	.command('server')
	.description('run server integration tests')
	.option('--with-server')
	.action(async (cmd: Command) => {
		cmd.withServer ? await withServer(server) : await server();
	});

command
	.command('rust-sdk')
	.description('run rust SDK integration tests')
	.option('--with-server')
	.action(async (cmd: Command) => {
		cmd.withServer ? await withServer(rustSDK) : await rustSDK();
	});
	
command
	.command('api')
	.description('run api integration tests')
	.option('--with-server')
	.action(async (cmd: Command) => {
		cmd.withServer ? await withServer(api) : await api();
	});
