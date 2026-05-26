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

async function fetchBlocks() {
    const response = await fetch(
        `${process.env.ZKSYNC_REST_ADDR}/api/v0.2/blocks?from=latest&limit=100&direction=older`
    );
    const data = await response.json();
    return data.result.list;
}

function saveBlocksToFile(blocks) {
    fs.writeFileSync('blocks.json', JSON.stringify(blocks));
}

function loadBlocksFromFile() {
    try {
        const fileContent = fs.readFileSync('blocks.json', 'utf-8');
        return JSON.parse(fileContent);
    } catch (error) {
        throw new Error('Unable to load blocks from file');
    }
}

async function main() {
    const blocks = await fetchBlocks();
    const results = blocks.reduce((acc, block) => {
        acc[block.blockNumber.toString()] = block.newStateRoot;
        return acc;
    }, {});

    let jsonBlocks;
    try {
        jsonBlocks = loadBlocksFromFile();
    } catch (e) {
        saveBlocksToFile(results);
        console.log('Blocks file created.');
        return;
    }

    for (const jsonBlockNum in jsonBlocks) {
        const rootHash = jsonBlocks[jsonBlockNum];
        if (rootHash !== results[jsonBlockNum]) {
            throw new Error(`Wrong block ${jsonBlockNum}`);
        }
    }
    console.log('Everything is fine');
}

(async () => {
    try {
        await main();
    } catch (error) {
        console.error(error);
    }
})();
