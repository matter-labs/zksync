import { Command } from 'commander';
import { Client } from 'ts-postgres';
import { ethers, BigNumber } from 'ethers';
import { utils } from 'zksync';

const ABI = [
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
                internalType: 'struct Storage.StoredBlockInfo[]',
                name: '_blocksToRevert',
                type: 'tuple[]'
            }
        ],
        name: 'revertBlocks',
        outputs: [],
        stateMutability: 'nonpayable',
        type: 'function'
    },
    {
        anonymous: false,
        inputs: [
            {
                indexed: false,
                internalType: 'uint32',
                name: 'totalBlocksVerified',
                type: 'uint32'
            },
            {
                indexed: false,
                internalType: 'uint32',
                name: 'totalBlocksCommitted',
                type: 'uint32'
            }
        ],
        name: 'BlocksRevert',
        type: 'event'
    }
];

function isProcessableOnchainOperation(type: string) {
    return type === 'Withdraw' || type === 'FullExit' || type === 'ForcedExit';
}

function getPubData(tx: any) {
    console.log(tx);
    if (tx.type === 'Withdraw') {
        const type = new Uint8Array([3]);
        const accountId = utils.serializeAccountId(tx.accountId);
        const token = utils.serializeTokenId(tx.token);
        const amount = utils.serializeAmountFull(BigNumber.from(tx.amount));
        const fee = utils.serializeFeePacked(utils.closestPackableTransactionFee(BigNumber.from(tx.fee)));
        const to = utils.serializeAddress(tx.to);
        return ethers.utils.concat([type, accountId, token, amount, fee, to, new Uint8Array(9)]);
    } else if (tx.type === 'FullExit') {
        const type = new Uint8Array([6]);
        const accountId = utils.serializeAccountId(tx.priority_op.account_id);
        const ethAddress = utils.serializeAddress(tx.priority_op.eth_address);
        const token = utils.serializeTokenId(tx.priority_op.token);
        const amount = utils.serializeAmountFull(tx.withdraw_amount ? tx.withdraw_amount : BigNumber.from(0));
        return ethers.utils.concat([type, accountId, ethAddress, token, amount, new Uint8Array(11)]);
    } else if (tx.type === 'ForcedExit') {
        const type = new Uint8Array([8]);
        const accountId = utils.serializeAccountId(tx.initiatorAccountId);
        //targetAccountId
        const token = utils.serializeTokenId(tx.token);
        
    }
    else {
        throw('Unknown operation type');
    }
}

async function revertBlocks(blocksNumber: number) {
    const ethProvider = ethers.getDefaultProvider(process.env.ETH_CLIENT_WEB3_URL);
    const ethWallet = new ethers.Wallet(process.env.ETH_SENDER_SENDER_OPERATOR_PRIVATE_KEY).connect(ethProvider);
    const contract = new ethers.Contract(process.env.CONTRACTS_CONTRACT_ADDR, ABI, ethWallet);
    const client = new Client({ user: 'postgres', database: 'plasma' });
    let blocksInfo = [];
    try {
        await client.connect();
        const blocks = await client.query(
            `SELECT number, root_hash, unprocessed_prior_op_before, unprocessed_prior_op_after, timestamp, commitment 
            FROM blocks
            WHERE number = 16
            ORDER BY number DESC
            LIMIT $1`,
            [blocksNumber]
        );
        for (const block of blocks) {
            const priorityOperations = BigNumber.from(block.get('unprocessed_prior_op_after')).sub(
                BigNumber.from(block.get('unprocessed_prior_op_before'))
            );
            let hash = ethers.utils.keccak256(new Uint8Array());
            const executed_p_txs = await client.query(
                `SELECT operation 
                FROM executed_priority_operations
                WHERE block_number = $1`,
                [block.get('number')]
            );
            for (const row of executed_p_txs) {
                const tx = row.get('operation') as any;
                if (isProcessableOnchainOperation(tx.type)) {
                    hash = ethers.utils.keccak256(ethers.utils.concat([ethers.utils.arrayify(hash), getPubData(tx)]));
                }
            }
            const executed_txs = await client.query(
                `SELECT tx 
                FROM executed_transactions
                WHERE block_number = $1`,
                [block.get('number')]
            );
            // console.log(executed_txs);
            
            for (const row of executed_txs) {
                const tx = row.get('tx') as any;
                if (isProcessableOnchainOperation(tx.type)) {
                    hash = ethers.utils.keccak256(ethers.utils.concat([ethers.utils.arrayify(hash), getPubData(tx)]));
                }
            }
            let blockInfo = {
                blockNumber: block.get('number').toString(),
                priorityOperations: priorityOperations.toString(),
                pendingOnchainOperationsHash: hash,
                timestamp: block.get('timestamp').toString(),
                stateHash: '0x' + (block.get('root_hash') as Buffer).toString('hex'),
                commitment: '0x' + (block.get('commitment') as Buffer).toString('hex')
            };
            blocksInfo.push(blockInfo);
        }
    } catch (e) {
        console.log(e);
    }

    contract.on('BlocksRevert', (totalBlocksVerified, totalBlocksCommitted, _event) => {
        console.log(totalBlocksVerified, totalBlocksCommitted);
    });
    console.log(blocksInfo);
    // let res = await contract.revertBlocks(blocksInfo, {
    //     gasLimit: '10000000'
    // });
    // await res.wait();
    await client.end();
}

async function main() {
    const program = new Command();
    program.version('0.1.0').name('revert-tool').description('revert blocks');

    program.command('revert <blocksNumber>').description('revert blocks').action(revertBlocks);

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
