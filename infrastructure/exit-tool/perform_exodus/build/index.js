"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    Object.defineProperty(o, k2, { enumerable: true, get: function() { return m[k]; } });
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
Object.defineProperty(exports, "__esModule", { value: true });
const commander_1 = require("commander");
const ethers_1 = require("ethers");
const fs = __importStar(require("fs"));
const program = new commander_1.Command();
program.version('0.0.1');
program
    .option('-pk, --private-key <private-key>', 'private key of the account')
    .option('-t, --target <target>', 'address of the zkSync account')
    .option('-n, --network <network>', 'eth network')
    .option('-p, --path <path/to/input>', 'path to the file with input for exodus');
program.parse(process.argv);
function getProvider(network) {
    if (network === 'localhost') {
        return new ethers_1.ethers.providers.JsonRpcProvider('http://localhost:8545');
    }
    return ethers_1.ethers.providers.getDefaultProvider(network);
}
const abi = [
    {
        "inputs": [
            {
                "components": [
                    {
                        "internalType": "uint32",
                        "name": "blockNumber",
                        "type": "uint32"
                    },
                    {
                        "internalType": "uint64",
                        "name": "priorityOperations",
                        "type": "uint64"
                    },
                    {
                        "internalType": "bytes32",
                        "name": "pendingOnchainOperationsHash",
                        "type": "bytes32"
                    },
                    {
                        "internalType": "uint256",
                        "name": "timestamp",
                        "type": "uint256"
                    },
                    {
                        "internalType": "bytes32",
                        "name": "stateHash",
                        "type": "bytes32"
                    },
                    {
                        "internalType": "bytes32",
                        "name": "commitment",
                        "type": "bytes32"
                    }
                ],
                "internalType": "struct Storage.StoredBlockInfo",
                "name": "_storedBlockInfo",
                "type": "tuple"
            },
            {
                "internalType": "address",
                "name": "_owner",
                "type": "address"
            },
            {
                "internalType": "uint32",
                "name": "_accountId",
                "type": "uint32"
            },
            {
                "internalType": "uint16",
                "name": "_tokenId",
                "type": "uint16"
            },
            {
                "internalType": "uint128",
                "name": "_amount",
                "type": "uint128"
            },
            {
                "internalType": "uint256[]",
                "name": "_proof",
                "type": "uint256[]"
            }
        ],
        "name": "performExodus",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {
                "internalType": "address payable",
                "name": "_owner",
                "type": "address"
            },
            {
                "internalType": "address",
                "name": "_token",
                "type": "address"
            },
            {
                "internalType": "uint128",
                "name": "_amount",
                "type": "uint128"
            }
        ],
        "name": "withdrawPendingBalance",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    }
];
async function main() {
    const { privateKey, target, path, network } = program;
    console.log("Starting the perform exodus script");
    const provider = getProvider(network || 'mainnet');
    const wallet = new ethers_1.Wallet(privateKey).connect(provider);
    console.log("Loading input file");
    const data = JSON.parse(fs.readFileSync(path, 'utf-8'));
    console.log("Input file loaded");
    const zkSyncContract = new ethers_1.ethers.Contract(target, abi, wallet);
    const storedBlockInfo = data["storedBlockInfo"];
    storedBlockInfo["timestamp"] = ethers_1.ethers.BigNumber.from(storedBlockInfo["timestamp"]);
    const owner = data["owner"];
    const accountId = data["accountId"];
    const tokenId = data["tokenId"];
    const tokenAddress = data["tokenAddress"];
    const amount = ethers_1.ethers.BigNumber.from(data["amount"]);
    const proof = data["proof"]["proof"].map((el) => ethers_1.ethers.BigNumber.from(el));
    console.log("Sending performExodus transaction");
    const exodusTx = await zkSyncContract.performExodus(storedBlockInfo, owner, accountId, tokenId, amount, proof, {
        gasLimit: 1000000,
    });
    console.log("performExodus sent, waiting for confirmation...");
    await exodusTx.wait();
    console.log("performExodus confirmed");
    console.log("Sending withdrawPendingBalance transaction");
    const withdrawTx = await zkSyncContract.withdrawPendingBalance(owner, tokenAddress, amount);
    console.log("withdrawPendingBalance sent, waiting for confirmation...");
    await withdrawTx.wait();
    console.log("withdrawPendingBalance confirmed");
    console.log("All done!");
}
(async () => {
    await main();
})();
