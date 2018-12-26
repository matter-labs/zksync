const ethers = require("ethers");
const abi_string = require("../build/contracts/PlasmaStorage.json").abi;
const assert = require("assert");
const transactionLib = require("../lib/transaction");
const ethUtils = require("ethereumjs-util");
const BN = require("bn.js");

// const rpcEndpoint = "http://127.0.0.1:8545";
// const contractAddress = "0x4169D71D56563eA9FDE76D92185bEB7aa1Da6fB8";

const rpcEndpoint = "https://rinkeby.infura.io/48beda66075e41bda8b124c6a48fdfa0";
const contractAddress = "0x2A8BadcC3d128d814AaEA66a89a6ba3e101D1761";

const blockNumber = 3;

async function getState() {
    let provider = new ethers.providers.JsonRpcProvider(rpcEndpoint);

    let contract = new ethers.Contract(contractAddress, abi_string, provider);
    const depositBatchSize = await contract.DEPOSIT_BATCH_SIZE();
    const totalDepositRequests = await contract.totalDepositRequests();
    console.log("Total deposits happened = " + totalDepositRequests.toString(10));
    const totalBatches = totalDepositRequests.div(depositBatchSize);
    console.log("Current batch = " + totalBatches.toString(10));

    const lastCommited = await contract.lastCommittedBlockNumber();
    console.log("Last committed block = " + lastCommited.toString(10));

    const lastVerified = await contract.lastVerifiedBlockNumber();
    console.log("Last verified block = " + lastVerified.toString(10));

    const lastVerifiedRoot = await contract.lastVerifiedRoot();
    console.log("Last verified root = " + lastVerifiedRoot.toString(16));

    const lastCommittedDepositBatch = await contract.lastCommittedDepositBatch();
    console.log("Last committed deposit batch = " + lastCommittedDepositBatch.toString(10));

    const lastVerifiedDepositBatch = await contract.lastVerifiedDepositBatch();
    console.log("Last verified deposit batch = " + lastVerifiedDepositBatch.toString(10));

    const block = await contract.blocks(blockNumber);
    console.log(JSON.stringify(block));
    console.log("Block data commitment = " + block.publicDataCommitment);

}

async function run() {
    await getState();
}

run().then()