import { exec as _exec, spawn as _spawn } from 'child_process';
import { promisify } from 'util';
import fs from 'fs';
import readline from 'readline';

export type { ChildProcess } from 'child_process';

const IGNORED_DIRS = [
    'target',
    'node_modules',
    'volumes',
    'build',
    'dist',
    '.git',
    'generated',
    'grafonnet-lib',
    'prettier-config',
    'lint-config',
    'cache',
    'artifacts',
    'typechain',
    'binaryen'
];
const IGNORED_FILES = ['KeysWithPlonkVerifier.sol', 'TokenInit.sol', '.tslintrc.js'];

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
            code == 0 ? resolve(code) : reject(`Child process exited with code ${code}`);
        });
    });
}

// executes a command in background and returns a child process handle
// by default pipes data to parent's stdio but this can be overridden
export function background(command: string, stdio: any = 'inherit') {
    command = command.replace(/\n/g, ' ');
    return _spawn(command, { stdio: stdio, shell: true, detached: true });
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

export async function sleep(seconds: number) {
    return new Promise((resolve) => setTimeout(resolve, seconds * 1000));
}

// the sync version of sleep is needed
// for process.on('exit') hook, which MUST be synchronous.
// no idea why it has to be so ugly, though
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

// performs an operation on the content of `filename`
export function modifyFile(filename: string, modifier: (s: string) => string) {
    const source = fs.readFileSync(filename).toString();
    fs.writeFileSync(filename, modifier(source));
}

// If you wonder why this is written so obscurely through find and not through .prettierignore and globs,
// it's because prettier *first* expands globs and *then* applies ignore rules, which leads to an error
// because it can't expand into volumes folder with not enough access rights, even if it is ignored.
//
// And if we let the shell handle glob expansion instead of prettier, `shopt -s globstar` will be
// disabled (because yarn spawns its own shell that does not load .bashrc) and thus glob patterns
// with double-stars will not work
export async function getUnignoredFiles(extension: string) {
    const root = extension == 'sol' ? 'contracts' : '.';
    const ignored_dirs = IGNORED_DIRS.map((dir) => `-o -path '*/${dir}' -prune`).join(' ');
    const ignored_files = IGNORED_FILES.map((file) => `-a ! -name '${file}'`).join(' ');
    const { stdout: files } = await exec(
        `find ${root} -type f -name '*.${extension}' ${ignored_files} -print ${ignored_dirs}`
    );

    return files;
}

export function web3Url() {
    // @ts-ignore
    return process.env.ETH_CLIENT_WEB3_URL.split(',')[0] as string;
}

export async function readZkSyncAbi() {
    const zksync = process.env.ZKSYNC_HOME;
    const path = `${zksync}/contracts/artifacts/cache/solpp-generated-contracts/ZkSync.sol/ZkSync.json`;

    const fileContent = (await fs.promises.readFile(path)).toString();

    const abi = JSON.parse(fileContent).abi;

    return abi;
}
