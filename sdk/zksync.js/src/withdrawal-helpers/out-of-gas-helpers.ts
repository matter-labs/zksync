
import { BigNumberish, ethers, Contract } from 'ethers';
import { Address, TokenLike } from '../types'; 
import { SYNC_MAIN_CONTRACT_INTERFACE } from '../utils';
import { Provider } from '../';

interface WithdrawPendingBalanceParams {
    owner: Address,
    token: Address,
    amount: BigNumberish
}

async function getWithdrawPendingBalanceParams(
    syncProvider: Provider,
    syncContract: Contract,
    from: Address,
    token: TokenLike,
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
    from: Address,
    token: TokenLike,
    amount?: BigNumberish
): Promise<any> {
    const contractAddress = syncProvider.contractAddress.mainContract;

    const zksyncContract = new Contract(
        contractAddress,
        SYNC_MAIN_CONTRACT_INTERFACE,
        ethersWallet
    );

        
    const callParams = await getWithdrawPendingBalanceParams(
        syncProvider,
        zksyncContract,
        from,
        token,
        amount
    );

    return await zksyncContract.withdrawPendingBalance(
        callParams.owner,
        callParams.token,
        callParams.amount
    );
}
