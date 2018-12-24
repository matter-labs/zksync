const ethers = require("ethers");
const abi_string = require("../build/contracts/PlasmaStorage.json").abi;
const assert = require("assert");
const transactionLib = require("../lib/transaction");
const ethUtils = require("ethereumjs-util");
const BN = require("bn.js");

// const rpcEndpoint = "http://127.0.0.1:8545";
// const contractAddress = "0x4169D71D56563eA9FDE76D92185bEB7aa1Da6fB8";

const rpcEndpoint = "https://rinkeby.infura.io/48beda66075e41bda8b124c6a48fdfa0";
const contractAddress = "0xe8D0E0c58857f5bb83Ee8ccE2D42f144Bb7263aC";

async function getLogs(batchNumberString) {
    let provider = new ethers.providers.JsonRpcProvider(rpcEndpoint);

    let contract = new ethers.Contract(contractAddress, abi_string, provider);
    const depositBatchSize = await contract.DEPOSIT_BATCH_SIZE();
    const totalDepositRequests = await contract.totalDepositRequests();
    console.log("Total deposits happened = " + totalDepositRequests.toString(10));
    const totalBatches = totalDepositRequests.div(depositBatchSize);
    console.log("Current batch = " + totalBatches.toString(10));
    for (let i = 0; i < totalBatches.toNumber(); i++) {
        console.log("Trying to get all logs for deposit batch " + i);
        let filter = contract.filters.LogDepositRequest("0x" + (new BN(i)).toString(16), null, null);
        // need to explicitly set block range
        let fullFilter = {
            fromBlock: 1,
            toBlock: 'latest',
            address: filter.address,
            topics: filter.topics
        };
        let events = await provider.getLogs(fullFilter);
        console.log(JSON.stringify(events));
    }
}

async function run() {
    const args = process.argv.slice(2);
    const batchNumber = args[0];
    await getLogs(batchNumber);
}

run().then()