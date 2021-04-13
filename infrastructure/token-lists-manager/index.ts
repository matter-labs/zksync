import { Command } from 'commander';
import { diffTokenLists, TokenInfo } from '@uniswap/token-lists';
import { getTokenList } from 'reading-tool';
import fetch from 'node-fetch';
import * as readline from 'readline';
import * as fs from 'fs';
import * as path from 'path';

const configPath = path.join(process.env.ZKSYNC_HOME as string, `infrastructure/token-lists-manager`);
const tokenListConfig = JSON.parse(
    fs.readFileSync(`${configPath}/.token-lists-sources-config.json`, { encoding: 'utf-8' })
);

function saveTokenList(source: string, tokeList: TokenInfo[]) {
    const path = `${process.env.ZKSYNC_HOME as string}/etc/token-lists/${source}.json`;
    fs.writeFileSync(path, JSON.stringify(tokeList));
}

async function fetchFromSourceTokenList(source: string): Promise<TokenInfo[] | null> {
    try {
        const link = tokenListConfig[source];
        const response = await fetch(link);
        const tokenList = (await response.json()).tokens as TokenInfo[];

        return tokenList;
    } catch (err) {
        console.log('Failed to load new token list: ', err);
        return null;
    }
}

async function updateTokenList(source: string): Promise<boolean> {
    console.log(`Update ${source} token list`);

    const oldTokenList = getTokenList(source) as TokenInfo[];
    const newTokenList = await fetchFromSourceTokenList(source);

    console.log(`diff: `, diffTokenLists(oldTokenList, newTokenList));

    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    const input = (await new Promise((resolve) => {
        rl.question('Want to update tokens with such changes? (Y/n)\n', (input) => {
            rl.close();
            resolve(input);
        });
    })) as string;

    if (input.toLowerCase() != 'y') {
        return false;
    }

    saveTokenList(source, newTokenList);
    return true;
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('token-lists-manager');

    program
        .command('update <source>')
        .description('Update token list')
        .action(async (source: string) => {
            let success = await updateTokenList(source);

            if (success) {
                console.log(`${source} token list updated successfully`);
            } else {
                console.log(`Failed to update ${source} token list`);
            }
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
