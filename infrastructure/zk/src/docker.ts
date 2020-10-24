import { Command } from 'commander';
import * as utils from './utils';
import * as contract from './contract';

const IMAGES = ['server', 'prover', 'nginx', 'geth', 'dev-ticker', 'keybase', 'ci', 'fee-seller', 'rust'];

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
    if (image == 'nginx') {
        await utils.spawn('yarn --cwd infrastructure/explorer build');
    }
    if (image == 'server' || image == 'prover') {
        await contract.build();
        await contract.buildDev();
    }
    const { stdout: imageTag } = await utils.exec('git rev-parse --short HEAD');
    const latestImage = `-t matterlabs/${image}:latest`;
    const taggedImage = ['nginx', 'server', 'prover'].includes(image) ? `-t matterlabs/${image}:${imageTag}` : '';
    await utils.spawn(`DOCKER_BUILDKIT=1 docker build ${latestImage} ${taggedImage} -f ./docker/${image}/Dockerfile`);
}

export async function push(image: string) {
    if (!IMAGES.includes(image)) {
        throw new Error(`Wrong image name: ${image}`);
    }
    if (image == 'rust') {
        await push('server');
        await push('prover');
        return;
    }
    if (image == 'keybase') {
        image = 'keybase-secret';
    }
    const latestImage = `matterlabs/${image}:latest`;
    await utils.spawn(`docker push ${latestImage}`);
    if (['nginx', 'server', 'prover'].includes(image)) {
        const { stdout: imageTag } = await utils.exec('git rev-parse --short HEAD');
        const taggedImage =  `matterlabs/${image}:${imageTag}`;
        await utils.spawn(`docker push ${taggedImage}`);
    }
}

const command = new Command('docker')
    .description('docker management');

command
    .command('build <image>')
    .description('build docker image')
    .action(build);

command
    .command('push <image>')
    .description('build and push docker image')
    .action(async (image: string) => {
        await build(image);
        await push(image);
    });
