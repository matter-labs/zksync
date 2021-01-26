import { types, Provider, utils } from 'zksync'; 
import { BigNumberish, ethers, Contract } from 'ethers';

interface WithdrawPendingBalanceParams {
    owner: types.Address,
    token: types.Address,
    amount: BigNumberish
}

async function getWithdrawPendingBalanceParams(
    syncProvider: Provider,
    syncContract: Contract,
    from: types.Address,
    token: types.TokenLike,
    amount?: BigNumberish
): Promise<WithdrawPendingBalanceParams> {

    const tokenAddress = syncProvider.tokenSet.resolveTokenAddress(
        token
    );

    const withdrawAmount = amount 
        ? amount
        : await syncContract.getPendingBalance(
        from,
        tokenAddress
    );

    return {
        owner: from,
        token: tokenAddress,
        amount: withdrawAmount
    };
}

export async function withdrawPendingBalance(
    syncProvider: Provider,
    ethersWallet: ethers.Signer,
    from: types.Address,
    token: types.TokenLike,
    amount?: BigNumberish
) {
    const contractAddress = syncProvider.contractAddress.mainContract;

    const zksyncContract = new Contract(
        contractAddress,
        utils.SYNC_MAIN_CONTRACT_INTERFACE,
        ethersWallet
    );

        
    const callParams = await getWithdrawPendingBalanceParams(
        syncProvider,
        zksyncContract,
        from,
        token,
        amount
    );

    return zksyncContract.withdrawPendingBalance(
        callParams.owner,
        callParams.token,
        callParams.amount
    );
}
