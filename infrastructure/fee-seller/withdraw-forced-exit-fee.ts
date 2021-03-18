import { BigNumber, ethers } from 'ethers';
import { getZkSyncApiAddress } from './utils';
import fetch from 'node-fetch';

const ForcedExitContractAbi = require('./forced-exit-abi.json');

const requiredGasLimit = 40000;

const SENDER_PRIVATE_KEY = process.env.FORCED_EXIT_REQUESTS_SENDER_ETH_PRIVATE_KEY;
const WITHDRAWAL_THRESHOLD = process.env.FORCED_EXIT_REQUESTS_WITHDRAWAL_THRESHOLD;
const FEE_RECEIVER = process.env.FORCED_EXIT_REQUESTS_FEE_RECEIVER;

async function shouldWithdrawForcedExitFee(
    ethProvider: ethers.providers.Provider,
    contractAddress: string,
    gasPrice: BigNumber
): Promise<boolean> {
    const costOfGas = gasPrice.mul(requiredGasLimit);
    const contractBalance = await ethProvider.getBalance(contractAddress);

    const profit = contractBalance.sub(costOfGas);
    const threshold = BigNumber.from(WITHDRAWAL_THRESHOLD);

    return profit.gte(threshold);
}

// Withdraws the fee from the ForcedExit requests feature
export async function withdrawForcedExitFee(ethProvider: ethers.providers.Provider, ethNetwork: string) {
    const gasPrice = await ethProvider.getGasPrice();
    const featureStatus = await getStatus(ethNetwork);

    if (featureStatus.status === 'disabled') {
        console.log('Forced exit requests feature is disabled');
        return;
    }

    const contractAddress = featureStatus.forcedExitContractAddress;
    const shouldWithdraw = await shouldWithdrawForcedExitFee(ethProvider, contractAddress, gasPrice);

    if (!shouldWithdraw) {
        console.log('It is not feasible to withdraw Forced Exit requests fee');
        return;
    }

    const ethWallet = new ethers.Wallet(SENDER_PRIVATE_KEY).connect(ethProvider);
    const forcedExitContract = new ethers.Contract(contractAddress, ForcedExitContractAbi, ethWallet);

    try {
        console.log('Withdrawing funds from the forced exit smart contract');
        const tx = (await forcedExitContract.withdrawPendingFunds(FEE_RECEIVER, {
            gasPrice,
            gasLimit: requiredGasLimit
        })) as ethers.ContractTransaction;

        const receipt = await tx.wait();

        console.log('Tx hash:', receipt.transactionHash);
    } catch (e) {
        console.error('Failed to withdraw funds from the forced exit smart contract: ', e);
        // Even though we try to keep the forced exit requests functionality
        // as distant from the rest of the code as possible, if the script to withdraw the funds
        // fails, we might run into risk of the operator running out of money, so not terminating
        // here would be a security issue
        process.exit(1);
    } finally {
        console.log('The process of withdrawing forced exit withdrawal fee is complete.');
    }
}

interface StatusResponse {
    status: 'enabled' | 'disabled';
    requestFee: string;
    maxTokensPerRequest: number;
    recomendedTxIntervalMillis: number;
    forcedExitContractAddress: string;
}

async function getStatus(network: string) {
    const apiUrl = `${getZkSyncApiAddress(network)}/api/forced_exit_requests/v0.1`;
    const endpoint = `${apiUrl}/status`;

    const response = await fetch(endpoint);

    return (await response.json()) as StatusResponse;
}
