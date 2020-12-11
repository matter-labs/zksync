import { Command } from 'commander';
import * as toml from 'toml';
import * as fs from 'fs';
import * as path from 'path';
import deepExtend from 'deep-extend';
import { env } from 'process';

const CONFIG_FILES = [
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
    'private.toml',
];

async function getEnvironment(): Promise<string> {
    const environmentFilePath = path.join(env['ZKSYNC_HOME'] as string, 'etc', 'env', 'current');
    // Try to read environment from file.
    if (fs.existsSync(environmentFilePath)) {
        const environment = (await fs.promises.readFile(environmentFilePath)).toString().trim();
        if (environment === '') {
            return environment;
        }
    }

    // Fallback scenario: file doesn't exist or is empty.
    return 'dev';
}

function getConfigPath(environment: string, configName: string): string {
    return path.join(env['ZKSYNC_HOME'] as string, 'etc', 'env', environment, configName);
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

async function checkConfigExistence(environment: string) {
    const configFolder = path.join(env['ZKSYNC_HOME'] as string, 'etc', 'env', environment);

    if (fs.existsSync(configFolder)) {
        return;
    }

    // Folder doesn't exist.
    if (environment == 'dev') {
        // Copy configs from the `base` folder.
        await fs.mkdirSync(configFolder);

        for (const configFile of CONFIG_FILES) {
            const from = getConfigPath('base', configFile);
            const to = getConfigPath('dev', configFile);
            await fs.promises.copyFile(from, to);
        }
        return;
    }

    // Folder doesn't exist and the environment is not `dev`.
    console.error(`Configuration files were not found for environment <${environment}>`);
    process.exit(1);
}

export async function loadAllConfigs(environment?: string) {
    if (!environment) {
        environment = await getEnvironment();
    }

    // Check that config folder exists (or initialize it).
    await checkConfigExistence(environment);

    // Accumulator to which we will load all the configs.
    let config = {};

    for (const configFile of CONFIG_FILES) {
        const localConfig = await loadConfig(environment, configFile);

        // Extend the `config` with the new values.
        deepExtend(config, localConfig);
    }

    console.log(`${JSON.stringify(config, null, 2)}`);
}

export const command = new Command('config').description('config management');

command
    .command('load [environment]')
    .description('load the config for a certain environment')
    .action(loadAllConfigs);
