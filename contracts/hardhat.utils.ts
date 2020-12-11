import * as fs from 'fs';
import * as path from 'path';

export type Network = 'localhost' | 'mainnet' | 'ropsten' | 'rinkeby' | 'test';

export function loadDefs(network: Network) {
    try {
        const configPath = path.join(process.env.ZKSYNC_HOME, `etc/contracts`);
        const config = JSON.parse(fs.readFileSync(`${configPath}/${network}.json`, { encoding: 'utf-8' }));
        const dummyVerifierConfig = network == 'localhost' || network == 'test' ? process.env.DUMMY_VERIFIER : false;

        return Object.assign(config, { DUMMY_VERIFIER: dummyVerifierConfig });
    } catch (err) {
        console.warn('Invalid Configuration file');
        return;
    }
}
