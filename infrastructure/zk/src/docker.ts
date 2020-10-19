import { Command } from 'commander';
import * as utils from './utils';

const IMAGES = [
    'server',
    'prover',
    'nginx',
    'geth',
    'dev-ticker',
    'keybase',
    'ci',
    'fee-seller',
    'rust'
];

export async function build(image: string) {
    if (!IMAGES.includes(image)) {
        throw new Error(`Wrong image name: ${image}`);
    }
    if (image == 'rust') {
        await build('server');
        await build('prover');
        return;
    }
    if (image == 'keybase') {
        image = 'keybase-secret';
    }
    const { stdout: imageTag } = await utils.exec('git rev-parse --short HEAD');
    const latestImage = `-t matterlabs/${image}:latest`;
    const taggedImage = ['nginx', 'server', 'prover'].includes(image) ? `-t matterlabs/${image}:${imageTag}` : '';
    await utils.spawn(`DOCKER_BUILDKIT=1 docker build ${latestImage} ${taggedImage} -f ./docker/${image}/Dockerfile`);
}

export async function push(image: string) {

}

const docker = new Command('docker').description('docker management');

docker
    .command('build <image>')
    .description('build docker image')
    .action(build);

docker
    .command('push <image>')
    .description('build and push docker image')
    .action(async (image: string) => {
        await build(image);
        await push(image);
    });

