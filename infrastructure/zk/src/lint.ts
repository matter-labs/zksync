import { Command } from 'commander';
import * as utils from './utils';

const LINT_COMMANDS = {
    md: 'markdownlint',
    sol: 'solhint',
    js: 'eslint', 
    ts: 'eslint --ext ts'
    // This is needed to silence typescipt. It is possible to create type 
    // guards, but unfortunately they would have rather weird type, so 
    // Record<string, string> is a better solution.
} as Record<string, string>;
const EXTENSIONS = Object.keys(LINT_COMMANDS);
const CONFIG_PATH = 'etc/lint-config';

export async function lint(extension: string, check: boolean = false) {
    if (!EXTENSIONS.includes(extension)) {
        throw new Error('Unsupported extension');
    }

    const files = await utils.getUnignoredFiles(extension);
    const command = LINT_COMMANDS[extension];
    const fixOption = check ? '' : '--fix';

    await utils.spawn(`yarn --silent ${command} ${fixOption} --config ${CONFIG_PATH}/${extension}.js ${files}`);
}

export const command = new Command('lint')
    .description('lint non-rust code')
    .option('--check')
    .arguments('[extension]')
    .action(async (extension: string | null) => {
        if (extension) {
            await lint(extension);
        } else {    
            for (const ext of EXTENSIONS) {
                await lint(ext);
            }
        }
    });
