import { Command } from 'commander';
import * as utils from './utils';

const EXTENSIONS = ['ts', 'md', 'sol', 'js', 'vue'];
const CONFIG_PATH = 'etc/prettier-config';

export async function prettier(extension: string, check: boolean = false) {
    if (!EXTENSIONS.includes(extension)) {
        throw new Error('Unsupported extension');
    }

    const command = check ? 'check' : 'write';
    const files = await utils.getUnignoredFiles(extension);

    await utils.spawn(`yarn --silent prettier --config ${CONFIG_PATH}/${extension}.js --${command} ${files}`);
}

export async function rustfmt(check: boolean = false) {
    process.chdir(process.env.ZKSYNC_HOME as string);
    const command = check ? 'cargo fmt -- --check' : 'cargo fmt';

    await utils.spawn(command);

    process.chdir('sdk/zksync-crypto');
    await utils.spawn(command);
    process.chdir(process.env.ZKSYNC_HOME as string);
}

export const command = new Command('fmt')
    .description('format code with prettier & rustfmt')
    .option('--check')
    .arguments('[extension]')
    .action(async (extension: string | null, cmd: Command) => {
        if (extension) {
            if (extension == 'rust') {
                await rustfmt(cmd.check);
            } else {
                await prettier(extension, cmd.check);
            }
        } else {
            for (const ext of EXTENSIONS) {
                await prettier(ext, cmd.check);
            }
            await rustfmt(cmd.check);
        }
    });

command
    .command('prettier')
    .option('--check')
    .arguments('[extension]')
    .action(async (extension: string | null, cmd: Command) => {
        if (extension) {
            await prettier(extension, cmd.check);
        } else {
            for (const ext of EXTENSIONS) {
                await prettier(ext, cmd.check);
            }
        }
    });

command
    .command('rustfmt')
    .option('--check')
    .action(async (cmd: Command) => {
        await rustfmt(cmd.check);
    });
