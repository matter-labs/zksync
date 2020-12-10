import * as fs from 'fs';
import * as path from 'path';

export type Network = 'localhost' | 'mainnet' | 'ropsten' | 'rinkeby' | 'test';

export function loadDefs(network: Network) {
    try {
        const configPath = path.join(process.env.ZKSYNC_HOME, `etc/contracts`);

        return JSON.parse(fs.readFileSync(`${configPath}/${network}.json`, { encoding: 'utf-8' }));
    } catch (err) {
        console.warn('Invalid Configuration file');
        return;
    }
}
