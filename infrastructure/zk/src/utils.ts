import { exec as _exec, spawn as _spawn } from 'child_process';
import { promisify } from 'util';
import fs from 'fs';
import dotenv from 'dotenv';

// async executor of shell commands
// spawns a new shell and can execute arbitrary commands, like "ls -la | grep .env"
// returns { stdout, stderr }
export const exec = promisify(_exec);

// executes a signle command in a new process
// pipes data to parent's stdout/stderr
export function spawn(command: string) {
    const child = _spawn(command, { stdio: 'inherit', shell: true });
    return new Promise((resolve, reject) => {
        child.on('error', reject);
        child.on('close', (code) => {
            code == 0 ? resolve() : reject(`Child process exited with code ${code}`);
        });
    });
}

// loads environment variables
export function loadEnv() {
    const current = 'etc/env/current';
    const ZKSYNC_ENV = process.env.ZKSYNC_ENV || (fs.existsSync(current) ? fs.readFileSync(current).toString() : 'dev');
    const ENV_FILE = `etc/env/${ZKSYNC_ENV}.env`;
    if (ZKSYNC_ENV == 'dev' && !fs.existsSync('etc/env/dev.env')) {
        fs.copyFileSync('etc/env/dev.env.example', 'etc/env/dev.env');
    }
    if (!fs.existsSync(ENV_FILE)) {
        throw new Error('ZkSync config file not found: ' + ENV_FILE);
    }
    process.env.ZKSYNC_ENV = ZKSYNC_ENV;
    process.env.ENV_FILE = ENV_FILE;
    dotenv.config({ path: ENV_FILE });
}
