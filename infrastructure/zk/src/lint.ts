import { Command } from 'commander';
import * as utils from './utils';

// Note that `rust` is not noted here, as clippy isn't run via `yarn`.
// `rust` option is still supported though.
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
    if (extension == 'rust') {
        await clippy();
        return;
    }

    if (!EXTENSIONS.includes(extension)) {
        throw new Error('Unsupported extension');
    }

    const files = await utils.getUnignoredFiles(extension);
    const command = LINT_COMMANDS[extension];
    const fixOption = check ? '' : '--fix';

    await utils.spawn(`yarn --silent ${command} ${fixOption} --config ${CONFIG_PATH}/${extension}.js ${files}`);
}

async function clippy() {
    // We don't want clippy to require running database.
    process.env.SQLX_OFFLINE = 'true';
    process.chdir(process.env.ZKSYNC_HOME as string);
    await utils.spawn('cargo clippy  --all --tests --benches -- -D warnings -A clippy::upper-case-acronyms');
    delete process.env.SQLX_OFFLINE;

    process.chdir('sdk/zksync-crypto');
    await utils.spawn('cargo clippy  --all --tests --benches -- -D warnings -A clippy::upper-case-acronyms');
    process.chdir(process.env.ZKSYNC_HOME as string);
}

export const command = new Command('lint')
    .description('lint code')
    .option('--check')
    .arguments('[extension]')
    .action(async (extension: string | null, cmd: Command) => {
        if (extension) {
            await lint(extension, cmd.check);
        } else {
            for (const ext of EXTENSIONS) {
                await lint(ext, cmd.check);
            }
            await clippy();
        }
    });
