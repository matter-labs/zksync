const ethers = require("ethers");

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

const parentPid = process.argv[2];

async function providerReady() {
    for (let numRetries = 0; numRetries < 15; ++numRetries) {
        try {
            console.log('connecting to provider...');
            const ethersProvider = new ethers.providers.JsonRpcProvider('http://localhost:7545');
            await ethersProvider.getBlockNumber();
            return ethersProvider;
        } catch (e) {
            if (e.toString().includes('invalid response - 0')) {
                await sleep(1000);
            } else {
                throw e;
            }
        }
    }

    throw new Error("Couldn't await provider.");
}

async function generateBlocks() {
    const ethersProvider = await providerReady();
    const blockGenerationIntervalMillis = 100;
    while (true) {
        await ethersProvider.send("evm_mine", []);
        await sleep(blockGenerationIntervalMillis);
    }
}

generateBlocks()
    .catch(e => {
        console.error(e);
        process.kill(parentPid);
    });
