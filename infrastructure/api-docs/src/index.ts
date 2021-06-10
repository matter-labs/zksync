import { Command, program } from 'commander';
import * as path from 'path';
import { compileApibForTest, compileApibForDocumentation, getDirPath } from './compile';
import { spawn } from './utils';

export const compile = new Command('compile')
    .description('compile .apib files')
    .option('--test', 'build test.apib')
    .action(async (cmd: Command) => {
        if (cmd.test) {
            await compileApibForTest();
        } else {
            await compileApibForDocumentation();
        }
    });

export const generateDocs = new Command('generate-docs')
    .description('generate docs .html file')
    .action(async (_cmd: Command) => {
        const pathToApib = path.join(getDirPath(), 'blueprint/documentation.apib');
        await spawn(`aglio -i ${pathToApib} -o index.html`);
    });

export const test = new Command('test').description('test docs').action(async (_cmd: Command) => {
    await spawn(`cd ${getDirPath()} && dredd`);
});

program.version('1.0.0').name('api-docs').description('api documentation tool');
program.addCommand(compile);
program.addCommand(generateDocs);
program.addCommand(test);

async function main() {
    await program.parseAsync(process.argv);
}

main().catch((err: Error) => {
    console.error('Error:', err.message || err);
    process.exitCode = 1;
});
