import { BigNumberish, ethers, Contract, BigNumber, utils, ContractTransaction } from 'ethers';
import { Address, TokenLike, Network } from './types';
import { SYNC_MAIN_CONTRACT_INTERFACE, MULTICALL_INTERFACE } from './utils';
import { Provider } from '.';

interface WithdrawPendingBalanceParams {
    owner: Address;
    token: Address;
    amount: BigNumberish;
}

interface MulticallParams {
    address?: Address;
    network?: Network;
    gasLimit?: BigNumberish;
}

async function getWithdrawPendingBalanceParams(
    syncProvider: Provider,
    syncContract: Contract,
    from: Address,
    token: TokenLike,
    amount?: BigNumberish
): Promise<WithdrawPendingBalanceParams> {
    const tokenAddress = syncProvider.tokenSet.resolveTokenAddress(token);

    const withdrawAmount = amount ? amount : await syncContract.getPendingBalance(from, tokenAddress);

    return {
        owner: from,
        token: tokenAddress,
        amount: withdrawAmount
    };
}

// The addresses are taken from here:
// https://github.com/makerdao/multicall
function getMulticallAddressByNetwork(network: Network) {
    switch (network) {
        case 'rinkeby':
        case 'rinkeby-beta':
            return '0x42ad527de7d4e9d9d011ac45b31d8551f8fe9821';
        case 'ropsten':
        case 'ropsten-beta':
            return '0x53c43764255c17bd724f74c4ef150724ac50a3ed';
        case 'mainnet':
            return '0xeefba1e63905ef1d7acba5a8513c70307c1ce441';
        default:
            throw new Error('There is no default multicall contract address for this network');
    }
}

export async function withdrawPendingBalance(
    syncProvider: Provider,
    ethersWallet: ethers.Signer,
    from: Address,
    token: TokenLike,
    amount?: BigNumberish
): Promise<ContractTransaction> {
    if (!ethersWallet.provider) {
        throw new Error('The Ethereum Wallet must be connected to a provider');
    }

    const contractAddress = syncProvider.contractAddress.mainContract;

    const zksyncContract = new Contract(contractAddress, SYNC_MAIN_CONTRACT_INTERFACE, ethersWallet);

    const gasPrice = await ethersWallet.provider.getGasPrice();

    const callParams = await getWithdrawPendingBalanceParams(syncProvider, zksyncContract, from, token, amount);

    return zksyncContract.withdrawPendingBalance(callParams.owner, callParams.token, callParams.amount, {
        gasLimit: BigNumber.from('200000'),
        gasPrice
    }) as ContractTransaction;
}

export async function withdrawPendingBalances(
    syncProvider: Provider,
    ethersWallet: ethers.Signer,
    addresses: Address[],
    tokens: TokenLike[],
    multicallParams: MulticallParams,
    amounts?: BigNumberish[]
): Promise<ContractTransaction> {
    if (tokens.length != addresses.length) {
        throw new Error('The array of addresses and the tokens should be the same length');
    }

    const multicallAddress = multicallParams.address || getMulticallAddressByNetwork(multicallParams.network);

    const contractAddress = syncProvider.contractAddress.mainContract;
    const zksyncContract = new Contract(contractAddress, SYNC_MAIN_CONTRACT_INTERFACE, ethersWallet);
    const gasPrice = await ethersWallet.provider.getGasPrice();

    const tokensAddresses = tokens.map((token) => syncProvider.tokenSet.resolveTokenAddress(token));

    if (!amounts) {
        const pendingWithdrawalsPromises = addresses.map((address, i) =>
            zksyncContract.getPendingBalance(address, tokensAddresses[i])
        );
        amounts = await Promise.all(pendingWithdrawalsPromises);
    }

    if (amounts.length != tokens.length) {
        throw new Error('The amounts array should be the same length as tokens array');
    }

    const calls = addresses.map((address, i) => {
        const callData = zksyncContract.interface.encodeFunctionData('withdrawPendingBalance', [
            address,
            tokensAddresses[i],
            amounts[i]
        ]);

        return [zksyncContract.address, callData];
    });

    const multicallContract = new Contract(multicallAddress, MULTICALL_INTERFACE, ethersWallet);

    return multicallContract.aggregate(calls, {
        gasLimit: multicallParams.gasLimit || BigNumber.from('300000'),
        gasPrice
    }) as ContractTransaction;
}
