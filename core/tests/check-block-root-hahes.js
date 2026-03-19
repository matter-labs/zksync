// Script only for testing reverting blocks.
// If the file with block number and root hashes does not exist,
// that means it's the first run of the script and we just fill the file.
// Otherwise we load this file and verify root hashes of the blocks.

const fs = require('fs');

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
    const blocks = data.result.list;
    const results = {};

    for (const block of blocks) {
        results[block.blockNumber.toString()] = block.newStateRoot;
    }

    let jsonBlocks;
    try {
        jsonBlocks = loadBlocksFile();
    } catch (_error) {
        fs.writeFileSync('blocks.json', JSON.stringify(results));
        return;
    }

    for (const jsonBlockNum of Object.keys(jsonBlocks)) {
        const rootHash = jsonBlocks[jsonBlockNum];
        if (rootHash !== results[jsonBlockNum]) {
            throw new Error(`Wrong block ${jsonBlockNum}`);
        }
    }

    console.log('Everything is fine');
}

main().catch((error) => {
    console.error(error);
    process.exit(1);
});
