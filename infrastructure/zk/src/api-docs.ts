import { Command } from 'commander';
import * as utils from './utils';

export async function buildDocs() {
    await utils.spawn('api_docs');
    await utils.spawn('api_docs compile');
    await utils.spawn('api_docs generate-docs');
}

export const command = new Command('api-docs').description('build api v0.2 documentation').action(buildDocs);
