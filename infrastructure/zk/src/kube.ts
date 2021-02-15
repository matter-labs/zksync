import { Command } from 'commander';
import * as utils from './utils';

export async function updateConfig() {
    let namespace = '';
    const env = process.env.ZKSYNC_ENV as string;
    const envFile = process.env.ENV_FILE;
    if (env == 'dev') {
        throw new Error('This command requires environment with k8s cluster');
    } else if (env == 'development') {
        namespace = 'dev';
    } else if (env == 'prod') {
        namespace = 'zksync';
    } else if (['stage', 'rinkeby', 'ropsten', 'breaking', 'rinkeby-beta', 'rospten-beta'].includes(env)) {
        namespace = env;
    } else {
        console.error('Unknown environment');
        return;
    }
    const configmap = `kubectl create configmap server-env-custom --from-env-file=${envFile} -n ${namespace} -o yaml --dry-run`;
    await utils.spawn(`${configmap} | kubectl diff -f - || true`);
    await utils.confirmAction();
    await utils.spawn(`${configmap} | kubectl apply -f -`);
}

export const command = new Command('kube').description('kubernetes management');

command.command('update-config').description('update kubernetes config').action(updateConfig);
