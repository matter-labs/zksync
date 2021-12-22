import { ethers } from 'ethers';
import { ParamType } from '@ethersproject/abi';
import * as chalk from 'chalk';

const warning = chalk.bold.yellow;

export function web3Url() {
    return process.env.ETH_CLIENT_WEB3_URL.split(',')[0] as string;
}

export function web3CustomProvider(url: string) {
    const provider = new ethers.providers.JsonRpcProvider(url);

    // Check that `CHAIN_ETH_NETWORK` variable is set. If not, it's most likely because
    // the variable was renamed. As this affects the time to deploy contracts in localhost
    // scenario, it surely deserves a warning.
    const network = process.env.CHAIN_ETH_NETWORK;
    if (!network) {
        console.log(warning('Network variable is not set. Check if contracts/scripts/utils.ts is correct'));
    }

    // Short polling interval for local network
    if (network === 'localhost') {
        provider.pollingInterval = 100;
    }

    return provider;
}
export function web3Provider() {
    return web3CustomProvider(web3Url());
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
