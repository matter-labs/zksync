const ethers = require("ethers");
const path = require("path");
const fs = require("fs");
const abi_string = fs.readFileSync(path.resolve(__dirname, "../bin/contracts_PlasmaTester_sol_PlasmaTester.abi"), 'UTF-8');
const assert = require("assert");
const transactionLib = require("../lib/transaction");
const ethUtils = require("ethereumjs-util");
const BN = require("bn.js");

// const rpcEndpoint = "http://127.0.0.1:8545";
// const contractAddress = "0x4169D71D56563eA9FDE76D92185bEB7aa1Da6fB8";
const rpcEndpoint = "https://rinkeby.infura.io/48beda66075e41bda8b124c6a48fdfa0";
const contractAddress = "0x2A8BadcC3d128d814AaEA66a89a6ba3e101D1761";

const privateKey = "0x12B7678FF12FE8574AB74FFD23B5B0980B64D84345F9D637C2096CA0EF587806";
const blockNumber = 2;

async function partialWithdraw() {
    let provider = new ethers.providers.JsonRpcProvider(rpcEndpoint);
    let walletWithProvider = new ethers.Wallet(privateKey, provider);
    if (process.env.MNEMONIC !== undefined) {
        console.log("Using mnemonics");
        walletWithProvider = ethers.Wallet.fromMnemonic(process.env.MNEMONIC);
        walletWithProvider = walletWithProvider.connect(provider);
    }
    const senderAddress = await walletWithProvider.getAddress();
    console.log("Sending from address " + senderAddress)
    let contract = new ethers.Contract(contractAddress, abi_string, walletWithProvider);
    const existingID = await contract.ethereumAddressToAccountID(senderAddress);
    console.log("This ethereum account has an id = " + existingID.toString(10));
    const balanceForWithdraw = await contract.partialExits(blockNumber, existingID);
    console.log("Balance for partial exit = " + balanceForWithdraw.toString(10));
    const lastVerifiedBlockNumber = await contract.lastVerifiedBlockNumber();
    console.log("Last verified block = " + lastVerifiedBlockNumber);
    const tx = await contract.withdrawPartialExitBalance(blockNumber);
    console.log("Result = ", tx.hash);
    const result = await tx.wait();
}

async function run() {
    await partialWithdraw();
}

run().then()