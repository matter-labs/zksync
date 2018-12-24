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
const contractAddress = "0xe8D0E0c58857f5bb83Ee8ccE2D42f144Bb7263aC";
// rinkeby 0xe8D0E0c58857f5bb83Ee8ccE2D42f144Bb7263aC
const privateKey = "0xaf355f2914da0ea0361a5f93ca4224f60ae6a00d648e63f8fdebacc26e5a06f3";

async function depositInto(acccountString, amountString) {
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
    const transactor = await contract.transactor();
    console.log("Transactor address  = " + transactor);
    const exitor = await contract.exitor();
    console.log("Exitor address = " + exitor);
    const nextAccountToRegister = await contract.nextAccountToRegister();
    console.log("Registering account = " + nextAccountToRegister.toString(10));
    const newKey = transactionLib.newKey();
    console.log("Plasma private key = " + newKey.privateKey.toString(16));
    let {x, y} = newKey.publicKey;
    x = "0x" + x.toString(16);
    y = "0x" + y.toString(16);
    const txAmount = ethers.utils.parseEther("0.001");
    console.log("Tx amount in wei = " + txAmount.toString(10));
    const tx = await contract.deposit([x, y], 0, {value: txAmount});
    console.log("Result = ", tx.hash);
    const result = await tx.wait();
    const totalDepositRequests = await contract.totalDepositRequests();
    console.log("Total deposits = " + totalDepositRequests.toString(10));
    const totalExitRequests = await contract.totalExitRequests();
    console.log("Total exits = " + totalExitRequests.toString(10));
}

async function run() {
    const args = process.argv.slice(2);
    const account = args[0];
    const amount = args[1];
    await depositInto(account, amount);
}

run().then()