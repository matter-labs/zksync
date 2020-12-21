import { Command } from 'commander';
import * as utils from './utils';

const IGNORED_DIRS = ['target', 'node_modules', 'volumes', 'build', 'dist', '.git', 'generated', 'grafonnet-lib'];
const IGNORED_FILES = ['KeysWithPlonkVerifier.sol', 'TokenInit.sol'];
const EXTENSIONS = ['ts', 'md', 'sol'];

// If you wonder why this is written so obscurely through find and not through .prettierignore and globs,
// it's because prettier *first* expands globs and *then* applies ignore rules, which leads to an error
// because it can't expand into volumes folder with not enough access rights, even if it is ignored.
//
// And if we let the shell handle glob expansion instead of prettier, `shopt -s globstar` will be
// disabled (because yarn spawns its own shell that does not load .bashrc) and thus glob patterns
// with double-stars will not work
export async function fmt(extension: string, check: boolean = false) {
    if (!EXTENSIONS.includes(extension)) {
        throw new Error('Unsupported extension');
    }
    const command = check ? 'check' : 'write';
    const root = extension == 'sol' ? 'contracts' : '.';
    const ignored_dirs = IGNORED_DIRS.map((dir) => `-o -path '*/${dir}' -prune`).join(' ');
    const ignored_files = IGNORED_FILES.map((file) => `-a ! -name '${file}'`).join(' ');
    const { stdout: files } = await utils.exec(
        `find ${root} -name '*.${extension}' ${ignored_files} -print ${ignored_dirs}`
    );
    await utils.spawn(`yarn --silent prettier --config .prettier-${extension}.json --${command} ${files}`);
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
