import { Command } from 'commander';
import { spawnSync } from 'child_process';

export const command = new Command('f')
    .arguments('<command...>')
    .allowUnknownOption()
    .action((command: string[]) => {
        const result = spawnSync(command[0], command.slice(1), { stdio: 'inherit' });
        if (result.error) {
            throw result.error;
        }
        process.exitCode = result.status || undefined;
    });

