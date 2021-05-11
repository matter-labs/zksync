import { ethers } from 'ethers';
import { ParamType } from '@ethersproject/abi';

export function web3Url() {
    return process.env.ETH_CLIENT_WEB3_URL.split(',')[0] as string;
}

export function web3Provider() {
    return new ethers.providers.JsonRpcProvider(web3Url());
}

export function storedBlockInfoParam(): ParamType {
    const StoredBlockInfoAbi = {
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
        name: '_lastCommittedBlockData',
        type: 'tuple'
    };

    return ParamType.fromObject(StoredBlockInfoAbi);
}
