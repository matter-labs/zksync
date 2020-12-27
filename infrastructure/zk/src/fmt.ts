import { Command } from 'commander';
import * as utils from './utils';

const EXTENSIONS = ['ts', 'md', 'sol', 'js', 'vue'];
const CONFIG_PATH = 'etc/prettier-config';

export async function fmt(extension: string, check: boolean = false) {
    if (!EXTENSIONS.includes(extension)) {
        throw new Error('Unsupported extension');
    }
    
    const command = check ? 'check' : 'write';
    const files = await utils.getUnignoredFiles(extension);

    await utils.spawn(`yarn --silent prettier --config ${CONFIG_PATH}/${extension}.js --${command} ${files}`);
}

export const command = new Command('fmt')
    .description('format code with prettier')
    .option('--check')
    .arguments('[extension]')
    .action(async (extension: string | null, cmd: Command) => {
        if (extension) {
            await fmt(extension, cmd.check);
        } else {
            for (const ext of EXTENSIONS) {
                await fmt(ext, cmd.check);
            }
        }
    });
