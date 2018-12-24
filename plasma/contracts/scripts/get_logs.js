const ethers = require("ethers");
const abi_string = require("../build/contracts/PlasmaStorage.json").abi;
const assert = require("assert");
const transactionLib = require("../lib/transaction");
const ethUtils = require("ethereumjs-util");
const BN = require("bn.js");

const rpcEndpoint = "http://127.0.0.1:8545";
const contractAddress = "0x4169D71D56563eA9FDE76D92185bEB7aa1Da6fB8";

async function getLogs(batchNumberString) {
    let provider = new ethers.providers.JsonRpcProvider(rpcEndpoint);

    let contract = new ethers.Contract(contractAddress, abi_string, provider);
    const depositBatchSize = await contract.DEPOSIT_BATCH_SIZE();
    const totalDepositRequests = await contract.totalDepositRequests();
    const totalBatches = totalDepositRequests.div(depositBatchSize);
    for (let i = 0; i < totalBatches.toNumber(); i++) {
        let filter = contract.filters.LogDepositRequest("0x" + (new BN(i)).toString(16), null, null);
        let events = await provider.getLogs(filter);
        console.log(JSON.stringify(events));
    }
}

async function run() {
    const args = process.argv.slice(2);
    const batchNumber = args[0];
    await getLogs(batchNumber);
}

run().then()