import { Command } from 'commander';
import * as utils from './utils';
import * as contract from './contract';

const IMAGES = [
    'server',
    'prover',
    'nginx',
    'dev-ticker',
    'keybase',
    'ci',
    'exit-tool',
    'dev-liquidity-token-watcher',
    'zk-environment',
    'event-listener',
    'data-restore'
    //,`'rskj'
];

async function dockerCommand(command: 'push' | 'build', image: string) {
    if (image == 'rust') {
        await dockerCommand(command, 'server');
        await dockerCommand(command, 'prover');
        return;
    }
    if (!IMAGES.includes(image)) {
        throw new Error(`Wrong image name: ${image}`);
    }
    if (image == 'keybase') {
        image = 'keybase-secret';
    }
    if (command == 'build') {
        await _build(image);
    } else if (command == 'push') {
        await _push(image);
    }
}

async function _build(image: string) {
    if (image == 'nginx') {
        await utils.spawn('yarn explorer build');
    }
    if (image == 'server' || image == 'prover') {
        await contract.build();
    }
    const { stdout: imageTag } = await utils.exec('git rev-parse --short HEAD');
    const latestImage = `-t matterlabs/${image}:latest`;
    const taggedImage = ['nginx', 'server', 'prover'].includes(image) ? `-t matterlabs/${image}:${imageTag}` : '';
    await utils.spawn(`DOCKER_BUILDKIT=1 docker build ${latestImage} ${taggedImage} -f ./docker/${image}/Dockerfile .`);
}

async function _push(image: string) {
    await utils.spawn(`docker push matterlabs/${image}:latest`);
    if (['nginx', 'server', 'prover', 'event-listener'].includes(image)) {
        const { stdout: imageTag } = await utils.exec('git rev-parse --short HEAD');
        await utils.spawn(`docker push matterlabs/${image}:${imageTag}`);
    }
}

export async function build(image: string) {
    await dockerCommand('build', image);
}

export async function push(image: string) {
    await dockerCommand('build', image);
    await dockerCommand('push', image);
}

export async function restart(container: string) {
    await utils.spawn(`docker-compose restart ${container}`);
}

export async function pull() {
    await utils.spawn('docker-compose pull postgres dev-liquidity-token-watcher dev-ticker tesseracts elastic');
}

export const command = new Command('docker').description('docker management');

command.command('build <image>').description('build docker image').action(build);
command.command('push <image>').description('build and push docker image').action(push);
command.command('pull').description('pull all containers').action(pull);
command.command('restart <container>').description('restart container in docker-compose.yml').action(restart);
