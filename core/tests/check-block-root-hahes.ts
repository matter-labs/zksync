// Script only for testing reverting blocks
// If the file with block number and root hashes does not exist,
// that means it's the first run of the script and we just fill the file
// Otherwise we have to load this file and verify root hashes of the blocks.
// It's not a production script, it's required only for integration test with reverting blocks

import * as fs from 'fs';
import fetch from 'node-fetch';

function loadBlocksFile() {
    return JSON.parse(
        fs.readFileSync('blocks.json', {
            encoding: 'utf-8'
        })
    );
}

async function main() {
    const response = await fetch(
        `${process.env.ZKSYNC_REST_ADDR}/api/v0.2/blocks?from=latest&limit=100&direction=older`
    );
    const data = await response.json();
    const blocks = data['result']['list'];
    let results: { [key: string]: string } = {};
    for (const block of blocks) {
        results[block['blockNumber'].toString()] = block['newStateRoot'];
    }
    let jsonBlocks;
    try {
        jsonBlocks = loadBlocksFile();
    } catch (e) {
        fs.writeFileSync('blocks.json', JSON.stringify(results));
        return;
    }
    for (const jsonBlockNum in jsonBlocks) {
        const rootHash = jsonBlocks[jsonBlockNum];
        if (rootHash !== results[jsonBlockNum]) {
            throw new Error(`Wrong block  ${jsonBlockNum}`);
        }
    }
    console.log('Everything is fine');
}

(async () => {
    await main();
})();
