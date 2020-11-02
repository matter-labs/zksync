import { Command } from 'commander';
import fs from 'fs';
import dotenv from 'dotenv';
import * as utils from './utils';

export function get() {
    fs.readdirSync('etc/env').forEach((file) => {
        if (file.endsWith('.bak') || file.endsWith('example') || file == 'current') {
            return;
        }
        const env = file.replace(/\..*$/, '');
        if (env == process.env.ZKSYNC_ENV) {
            console.log(' * ' + env);
        } else {
            console.log('   ' + env);
        }
    });
}

export async function gitHooks() {
    if (fs.existsSync('.git')) {
        await utils.exec(`
            git config --local core.hooksPath ||
            git config --local core.hooksPath ${process.env.ZKSYNC_HOME}/.githooks
        `);
    }
}

export function set(env: string) {
    const envFile = `etc/env/${env}.env`;
    if (!fs.existsSync(envFile)) {
        throw new Error(envFile + ' not found');
    }
    fs.writeFileSync('etc/env/current', env);
    process.env.ENV_FILE = envFile;
    process.env.ZKSYNC_ENV = env;
    get();
}

// we have to manually override the environment
// because dotenv won't override variables that are already set
export function reload() {
    const envFile = process.env.ENV_FILE as string;
    const env = dotenv.parse(fs.readFileSync(envFile));
    for (const envVar in env) {
        process.env[envVar] = env[envVar];
    }
}

// loads environment variables
export function load() {
    const current = 'etc/env/current';
    const zksyncEnv =
        process.env.ZKSYNC_ENV || (fs.existsSync(current) ? fs.readFileSync(current).toString().trim() : 'dev');
    const envFile = `etc/env/${zksyncEnv}.env`;
    if (zksyncEnv == 'dev' && !fs.existsSync('etc/env/dev.env')) {
        fs.copyFileSync('etc/env/dev.env.example', 'etc/env/dev.env');
    }
    if (!fs.existsSync(envFile)) {
        throw new Error('ZkSync config file not found: ' + envFile);
    }
    process.env.ZKSYNC_ENV = zksyncEnv;
    process.env.ENV_FILE = envFile;
    dotenv.config({ path: envFile });
}

// replaces an env variable in current .env file
// takes variable name, e.g. VARIABLE
// and the new assignment, e.g. VARIABLE=foo
export function modify(variable: string, assignedVariable: string) {
    const envFile = process.env.ENV_FILE as string;
    utils.replaceInFile(envFile, `${variable}=.*`, assignedVariable.trim());
    reload();
}

export const command = new Command('env')
    .arguments('[env_name]')
    .description('get or set zksync environment')
    .action((envName?: string) => {
        envName ? set(envName) : get();
    });
