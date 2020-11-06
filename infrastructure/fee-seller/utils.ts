import { BigNumber, BigNumberish, Contract, ethers, utils } from 'ethers';
import Axios from 'axios';
import * as zksync from 'zksync';

export async function getExpectedETHSwapResult(
    tokenSymbol: string,
    tokenDecimals: number,
    amount: BigNumberish
): Promise<BigNumber> {
    const coinGeckoResponse = await Axios.get(
        `https://api.coingecko.com/api/v3/simple/price?ids=${tokenSymbol}&vs_currencies=ETH`
    ).then((resp) => resp.data[tokenSymbol.toLowerCase()]['eth'].toString());
    const ethWeiPerTokenWei = utils.parseEther(coinGeckoResponse).div(BigNumber.from(10).pow(tokenDecimals));
    return ethWeiPerTokenWei.mul(amount);
}

export function numberAsFractionInBIPs(number: BigNumberish, baseFraction: BigNumberish): BigNumber {
    const base = BigNumber.from(baseFraction);
    if (base.eq(0)) {
        throw new Error("Base fraction can't be 0");
    }
    const num = BigNumber.from(number);
    if (num.lt(0) || base.lt(0)) {
        throw new Error('Numbers should be non-negative');
    }
    return num.mul(10000).div(base);
}

export function isOperationFeeAcceptable(
    balance: BigNumberish,
    withdrawFee: BigNumberish,
    operationFeeThreshold: number
): boolean {
    balance = BigNumber.from(balance);
    withdrawFee = BigNumber.from(withdrawFee);

    if (balance.eq(0)) {
        return false;
    }

    if (balance.lte(withdrawFee)) {
        return false;
    }

    return numberAsFractionInBIPs(withdrawFee, balance).lte(operationFeeThreshold * 100);
}

export async function approveTokenIfNotApproved(signer: ethers.Signer, tokenAddress: string, contractAddress: string) {
    const MAX_ERC20_APPROVE_AMOUNT = '115792089237316195423570985008687907853269984665640564039457584007913129639935'; // 2^256 - 1

    const ERC20_APPROVE_TRESHOLD = '57896044618658097711785492504343953926634992332820282019728792003956564819968'; // 2^255

    const IERC20_INTERFACE = [
        {
            constant: false,
            inputs: [
                {
                    name: 'spender',
                    type: 'address'
                },
                {
                    name: 'amount',
                    type: 'uint256'
                }
            ],
            name: 'approve',
            outputs: [
                {
                    name: '',
                    type: 'bool'
                }
            ],
            payable: false,
            stateMutability: 'nonpayable',
            type: 'function'
        },
        {
            constant: true,
            inputs: [
                {
                    name: 'owner',
                    type: 'address'
                },
                {
                    name: 'spender',
                    type: 'address'
                }
            ],
            name: 'allowance',
            outputs: [
                {
                    name: '',
                    type: 'uint256'
                }
            ],
            payable: false,
            stateMutability: 'view',
            type: 'function'
        }
    ];

    const erc20contract = new Contract(tokenAddress, IERC20_INTERFACE, signer);
    const currentAllowance = await erc20contract.allowance(await signer.getAddress(), contractAddress);
    const approved = BigNumber.from(currentAllowance).gte(ERC20_APPROVE_TRESHOLD);
    if (!approved) {
        console.log(`Approving token ${tokenAddress}`);
        const tx = await erc20contract.approve(contractAddress, MAX_ERC20_APPROVE_AMOUNT);
    }
}

export async function sendNotification(text: string, webhookUrl: string) {
    try {
        await Axios.post(webhookUrl, {
            username: 'fee_seller_bot',
            text
        });
    } catch (e) {
        console.error('Failed to send notification: ', e.toString());
    }
}

export function fmtToken(zksProvider: zksync.Provider, token, amount: BigNumber): string {
    return `${zksProvider.tokenSet.formatToken(token, amount)} ${zksProvider.tokenSet.resolveTokenSymbol(token)}`;
}

export async function fmtTokenWithETHValue(zksProvider: zksync.Provider, token, amount: BigNumber): Promise<string> {
    const tokenSymbol = zksProvider.tokenSet.resolveTokenSymbol(token);
    const tokenDecimals = zksProvider.tokenSet.resolveTokenDecimals(token);
    const estimatedETHValue = await getExpectedETHSwapResult(tokenSymbol, tokenDecimals, amount);
    return `${fmtToken(zksProvider, token, amount)} (${fmtToken(zksProvider, 'ETH', estimatedETHValue)})`;
}
