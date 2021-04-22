import { Command } from 'commander';
import { ethers, Wallet } from 'ethers';
import * as fs from 'fs';

const program = new Command();
program.version('0.0.1');

program
    .option('-pk, --private-key <private-key>', 'private key of the account')
    .option('-t, --target <target>', 'address of the zkSync account')
    .option('-n, --network <network>', 'eth network')
    .option('-p, --path <path/to/input>', 'path to the file with input for exodus');

program.parse(process.argv);

function getProvider(network: string) {
    if (network === 'localhost') {
        return new ethers.providers.JsonRpcProvider('http://localhost:8545');
    }

    return ethers.providers.getDefaultProvider(network);
}

const abi = [
    {
        inputs: [
            {
                components: [
                    {
                        internalType: 'uint32',
                        name: 'blockNumber',
                        type: 'uint32'
                    },
                    {
                        internalType: 'uint64',
                        name: 'priorityOperations',
                        type: 'uint64'
                    },
                    {
                        internalType: 'bytes32',
                        name: 'pendingOnchainOperationsHash',
                        type: 'bytes32'
                    },
                    {
                        internalType: 'uint256',
                        name: 'timestamp',
                        type: 'uint256'
                    },
                    {
                        internalType: 'bytes32',
                        name: 'stateHash',
                        type: 'bytes32'
                    },
                    {
                        internalType: 'bytes32',
                        name: 'commitment',
                        type: 'bytes32'
                    }
                ],
                internalType: 'struct Storage.StoredBlockInfo',
                name: '_storedBlockInfo',
                type: 'tuple'
            },
            {
                internalType: 'address',
                name: '_owner',
                type: 'address'
            },
            {
                internalType: 'uint32',
                name: '_accountId',
                type: 'uint32'
            },
            {
                internalType: 'uint16',
                name: '_tokenId',
                type: 'uint16'
            },
            {
                internalType: 'uint128',
                name: '_amount',
                type: 'uint128'
            },
            {
                internalType: 'uint256[]',
                name: '_proof',
                type: 'uint256[]'
            }
        ],
        name: 'performExodus',
        outputs: [],
        stateMutability: 'nonpayable',
        type: 'function'
    },
    {
        inputs: [
            {
                internalType: 'address payable',
                name: '_owner',
                type: 'address'
            },
            {
                internalType: 'address',
                name: '_token',
                type: 'address'
            },
            {
                internalType: 'uint128',
                name: '_amount',
                type: 'uint128'
            }
        ],
        name: 'withdrawPendingBalance',
        outputs: [],
        stateMutability: 'nonpayable',
        type: 'function'
    }
];

async function main() {
    const { privateKey, target, path, network } = program;

    console.log('Starting the perform exodus script');

    const provider = getProvider(network || 'mainnet');
    const wallet = new Wallet(privateKey).connect(provider);

    console.log('Loading input file');
    const data = JSON.parse(fs.readFileSync(path, 'utf-8'));
    console.log('Input file loaded');

    const zkSyncContract = new ethers.Contract(target, abi, wallet);

    const storedBlockInfo = data['storedBlockInfo'];
    storedBlockInfo['timestamp'] = ethers.BigNumber.from(storedBlockInfo['timestamp']);
    const owner = data['owner'];
    const accountId = data['accountId'];
    const tokenId = data['tokenId'];
    const tokenAddress = data['tokenAddress'];
    const amount = ethers.BigNumber.from(data['amount']);
    const proof = data['proof']['proof'].map((el: string) => ethers.BigNumber.from(el));

    console.log('Sending performExodus transaction');
    const exodusTx = await zkSyncContract.performExodus(storedBlockInfo, owner, accountId, tokenId, amount, proof, {
        gasLimit: 1_000_000
    });
    console.log('performExodus sent, waiting for confirmation...');

    await exodusTx.wait();
    console.log('performExodus confirmed');

    console.log('Sending withdrawPendingBalance transaction');
    const withdrawTx = await zkSyncContract.withdrawPendingBalance(owner, tokenAddress, amount, {
        gasLimit: 500_000
    });
    console.log('withdrawPendingBalance sent, waiting for confirmation...');
    await withdrawTx.wait();
    console.log('withdrawPendingBalance confirmed');

    console.log('All done!');
}

(async () => {
    await main();
})();
