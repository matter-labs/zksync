import { BigNumberish, ethers, Contract, BigNumber, ContractTransaction } from 'ethers';
import { Wallet } from './wallet';
import { Address, TokenLike, Network } from './types';
import { MULTICALL_INTERFACE } from './utils';

declare module './wallet' {
    interface Wallet {
        withdrawPendingBalance(from: Address, token: TokenLike, amount?: BigNumberish): Promise<ContractTransaction>;
        withdrawPendingBalances(
            addresses: Address[],
            tokens: TokenLike[],
            multicallParams: MulticallParams,
            amounts?: BigNumberish[]
        ): Promise<ContractTransaction>;
    }
}

interface MulticallParams {
    address?: Address;
    network?: Network;
    gasLimit?: BigNumberish;
}

function checkEthProvider(ethersWallet: ethers.Signer) {
    if (!ethersWallet.provider) {
        throw new Error('The Ethereum Wallet must be connected to a provider');
    }
}

// The addresses are taken from here:
// https://github.com/makerdao/multicall
function getMulticallAddressByNetwork(network: Network) {
    switch (network) {
        case 'testnet':
            return '0x9e469e1fc7fb4c5d17897b68eaf1afc9df39f103';
        case 'mainnet':
            return '0x6c62bf5440de2cb157205b15c424bceb5c3368f5';
        case 'mainnet-zk':
            return '0xeefba1e63905ef1d7acba5a8513c70307c1ce441';
        default:
            throw new Error('There is no default multicall contract address for this network');
    }
}

Wallet.prototype.withdrawPendingBalance = async function (
    // Here and in all the other functions in this file
    // "this" is just to make the `this` typed.
    // User do not have to pass it.
    this: Wallet,
    from: Address,
    token: TokenLike,
    amount?: BigNumberish
): Promise<ContractTransaction> {
    checkEthProvider(this.ethSigner);

    const zksyncContract = this.getZkSyncMainContract();

    const gasPrice = await this.ethSigner.getGasPrice();

    const tokenAddress = this.provider.tokenSet.resolveTokenAddress(token);
    const withdrawAmount = amount ? amount : await zksyncContract.getPendingBalance(from, tokenAddress);

    return zksyncContract.withdrawPendingBalance(from, tokenAddress, withdrawAmount, {
        gasLimit: BigNumber.from('200000'),
        gasPrice
    }) as ContractTransaction;
};

Wallet.prototype.withdrawPendingBalances = async function (
    this: Wallet,
    addresses: Address[],
    tokens: TokenLike[],
    multicallParams: MulticallParams,
    amounts?: BigNumberish[]
): Promise<ContractTransaction> {
    checkEthProvider(this.ethSigner);

    if (tokens.length != addresses.length) {
        throw new Error('The array of addresses and the tokens should be the same length');
    }

    const multicallAddress = multicallParams.address || getMulticallAddressByNetwork(multicallParams.network);

    const zksyncContract = this.getZkSyncMainContract();
    const gasPrice = await this.ethSigner.getGasPrice();

    const tokensAddresses = tokens.map((token) => this.provider.tokenSet.resolveTokenAddress(token));

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

    const multicallContract = new Contract(multicallAddress, MULTICALL_INTERFACE, this.ethSigner);

    return multicallContract.aggregate(calls, {
        gasLimit: multicallParams.gasLimit || BigNumber.from('300000'),
        gasPrice
    }) as ContractTransaction;
};
