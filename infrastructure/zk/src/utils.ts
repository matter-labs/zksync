import { exec as _exec, spawn as _spawn } from 'child_process';
import { promisify } from 'util';
import fs from 'fs';
import dotenv from 'dotenv';
import readline from 'readline';

export type { ChildProcess } from 'child_process';

// async executor of shell commands
// spawns a new shell and can execute arbitrary commands, like "ls -la | grep .env"
// returns { stdout, stderr }
const promisified = promisify(_exec);
export function exec(command: string) {
    command = command.replace(/\n/g, ' ');
    return promisified(command);
}

// executes a command in a new shell
// but pipes data to parent's stdout/stderr
export function spawn(command: string) {
    command = command.replace(/\n/g, ' ');
    const child = _spawn(command, { stdio: 'inherit', shell: true });
    return new Promise((resolve, reject) => {
        child.on('error', reject);
        child.on('close', (code) => {
            code == 0 ? resolve() : reject(`Child process exited with code ${code}`);
        });
    });
}

// executes a command in background and returns a child process handle
// by default pipes data to parent's stdio but this can be overriden
export function background(command: string, stdio: any = 'inherit') {
    command = command.replace(/\n/g, ' ');
    return _spawn(command, { stdio, shell: true, detached: true });
}

export async function confirmAction() {
    if (process.env.ZKSYNC_ACTION == 'dont_ask') return;
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });
    const input = await new Promise((resolve) => {
        rl.question(
            'Dangerous action! (set ZKSYNC_ACTION=dont_ask to always allow)\n' +
                `Type environment name (${process.env.ZKSYNC_ENV}) to confirm: `,
            (input) => {
                rl.close();
                resolve(input);
            }
        );
    });
    if (input !== process.env.ZKSYNC_ENV) {
        throw new Error('[aborted] action was not confirmed');
    }
}

function overrideEnv() {
    const envFile = process.env.ENV_FILE as string;
    const env = dotenv.parse(fs.readFileSync(envFile));
    for (const envVar in env) {
        process.env[envVar] = env[envVar];
    }
}

// loads environment variables
export function loadEnv() {
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
    overrideEnv();
}

// replaces an env variable in current .env file
// takes variable name, e.g. VARIABLE
// and the new assignment, e.g. VARIABLE=foo
export function modifyEnv(variable: string, assignedVariable: string) {
    const envFile = process.env.ENV_FILE as string;
    replaceInFile(envFile, `${variable}=.*`, assignedVariable.trim());
    overrideEnv();
}

export async function sleep(seconds: number) {
    return new Promise((resolve) => setTimeout(resolve, seconds * 1000));
}

export function sleepSync(seconds: number) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, seconds * 1000);
}

export async function allowFail<T>(promise: Promise<T>) {
    try {
        return await promise;
    } catch {
        return null;
    }
}

export function allowFailSync<T>(func: () => T) {
    try {
        return func();
    } catch {
        return null;
    }
}

export function replaceInFile(filename: string, before: string | RegExp, after: string) {
    before = new RegExp(before, 'g');
    modifyFile(filename, (source) => source.replace(before, after));
}

export function modifyFile(filename: string, modifier: (s: string) => string) {
    const source = fs.readFileSync(filename).toString();
    fs.writeFileSync(filename, modifier(source));
}
