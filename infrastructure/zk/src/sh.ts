import { exec as _exec, spawn as _spawn } from 'child_process';
import { promisify } from 'util';

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
