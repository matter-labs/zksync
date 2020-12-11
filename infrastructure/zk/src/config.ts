import * as toml from 'toml';
import * as fs from 'fs';
import * as path from 'path';
import { env } from 'process';

const configFiles = [
    'api.toml',
    'chain.toml',
    'contracts.toml',
    'database.toml',
    'eth_client.toml',
    'eth_sender.toml',
    'eth_watch.toml',
    'fee_ticker.toml',
    'misc.toml',
    'prover.toml',
    'rust.toml',
];

function getConfigPath(environment: string, configName: string): string {
    return path.join(env['ZKSYNC_HOME'], 'etc', 'env', environment, configName);
}

async function loadConfig(environment: string, configName: string) {
    const configPath = getConfigPath(environment, configName);
    const fileContents = await fs.promises.readFile(configPath);
    try {
        const tomlData = toml.parse(fileContents.toString());
        return tomlData;
    } catch (e) {
        console.error(
            `<${environment}/${configName}> load failed: Parsing error on line ${e.line} column ${e.column}: ${e.message}`
        );
        process.exit(1);
    }
}
