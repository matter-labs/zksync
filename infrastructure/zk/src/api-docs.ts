import { Command } from 'commander';
import * as utils from './utils';
import { compileForDocumentation } from 'api-docs';

export async function build_docs() {
    await compileForDocumentation();
    await utils.spawn('yarn api-docs build-docs');
}

export const command = new Command('api-docs').description('build api v0.2 documentation').action(build_docs);
