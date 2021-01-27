
import { BigNumberish, ethers, Contract, BigNumber } from 'ethers';
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

    if (!ethersWallet.provider) {
        throw new Error('The Ethereum Wallet must be connected to a provider');
    }

    const contractAddress = syncProvider.contractAddress.mainContract;

    const zksyncContract = new Contract(
        contractAddress,
        SYNC_MAIN_CONTRACT_INTERFACE,
        ethersWallet
    );

    const gasPrice = await ethersWallet.provider.getGasPrice();
        
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
        callParams.amount, 
        {
            gasLimit: BigNumber.from('200000'),
            gasPrice,
        }
    );
}
