import { Command } from 'commander';
import * as utils from './utils';

const LINT_COMMANDS = {
    md: 'markdownlint',
    sol: 'solhint',
    js: 'eslint',
    // This line is needed to make eslint ignore eslintrc.js when using .ts files.
    ts: 'eslint --ext ts --no-eslintrc -c .tslintrc.js'
    // This is needed to silence typescipt. It is possible to create type 
    // guards, but unfortunately they would have rather weird type, so 
    // Record<string, string> is a better solution.
} as Record<string, string>;
const EXTENSIONS = Object.keys(LINT_COMMANDS);

export async function lint(extension: string) {
    if (!EXTENSIONS.includes(extension)) {
        throw new Error('Unsupported extension');
    }

    const files = await utils.getUnignoredFiles(extension);
    const command = LINT_COMMANDS[extension];

    await utils.spawn(`yarn --silent ${command} ${files}`);
}

export const command = new Command('lint')
    .description('lint non-rust code')
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
