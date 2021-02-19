/**
 * Script that sells accumulated transaction fees for ETH using 1inch exchange and transfer ETH to other account.
 *
 * Selling is done in steps:
 *    Step 1 - token is withdrawn to the ETH account
 *    Step 2 - token is swapped for ETH using 1inch
 *    Step 3 - ETH is transferred to the OPERATOR_FEE_ETH_ADDRESS
 *
 *    Each step happens one after another without waiting for previous to complete
 *    so this script should be run frequently (e.g. once every 15 min).
 *
 * Sends all Ethereum transactions with sequential nonce starting with the next available not finalized nonce.
 * If the fee account already has a pending transaction with such a nonce, then it ignores it and replaces old the transaction.
 *
 * Each operation is performed only if transaction fee of this operation is less then MAX_LIQUIDATION_FEE_PERCENT.
 *
 * See Env parameters for available configuration parameters.
 */
import Axios from 'axios';
import { BigNumber, ethers } from 'ethers';
import * as zksync from 'zksync';
import {
    approveTokenIfNotApproved,
    fmtToken,
    fmtTokenWithETHValue,
    getExpectedETHSwapResult,
    isOperationFeeAcceptable,
    sendNotification
} from './utils';
import { EthParameters } from './types';

/** Env parameters. */
const FEE_ACCOUNT_PRIVATE_KEY = process.env.MISC_FEE_ACCOUNT_PRIVATE_KEY;
const MAX_LIQUIDATION_FEE_PERCENT = parseInt(process.env.MISC_MAX_LIQUIDATION_FEE_PERCENT);
const OPERATOR_FEE_ETH_ADDRESS = process.env.CHAIN_STATE_KEEPER_FEE_ACCOUNT_ADDR;
const ETH_NETWORK = process.env.CHAIN_ETH_NETWORK as any;
const WEB3_URL = process.env.ETH_CLIENT_WEB3_URL.split(',')[0];
const MAX_LIQUIDATION_FEE_SLIPPAGE = parseInt(process.env.MAX_LIQUIDATION_FEE_SLIPPAGE) || 5;
/** The threshold amount of the operator address to use reserve fee accumulator due to security reasons */
const THRESHOLD_AMOUNT_TO_USE_RESERVE_ADDRESS = process.env.THRESHOLD_AMOUNT_TO_USE_RESERVE_ADDRESS
    ? ethers.utils.parseEther(process.env.THRESHOLD_AMOUNT_TO_USE_RESERVE_ADDRESS)
    : ethers.utils.parseEther('25.0');
const RESERVE_FEE_ACCUMULATOR_ADDRESS = process.env.MISC_RESERVE_FEE_ACCUMULATOR_ADDRESS;
/** These assets will be transferred to the reserve fee accumulator address through the ZkSync network */
const ESTABLISHED_ASSETS_FOR_WITHDRAWING_THROUGH_ZKSYNC = process.env.MISC_ESTABLISHED_ASSETS_FOR_WITHDRAWING_THROUGH_ZKSYNC.split(
    ','
);
/** Amount of ETH that should be left on the fee account after third step. */
const ETH_TRANSFER_THRESHOLD = process.env.ETH_TRANSFER_THRESHOLD
    ? ethers.utils.parseEther(process.env.ETH_TRANSFER_THRESHOLD)
    : ethers.utils.parseEther('3.0');
/** Mattermost webhook url */
const NOTIFICATION_WEBHOOK_URL = process.env.NOTIFICATION_WEBHOOK_URL;

/** Approve ERC-20 tokens for this address */
const INCH_APPROVE = '0xe4c9194962532feb467dce8b3d42419641c6ed2e';
/** Send exchange tx to this address */
const INCH_EXCHANGE = '0x11111254369792b2Ca5d084aB5eEA397cA8fa48B';

/** Withdraw everything that has to be withdrawn */
async function withdrawTokens(zksWallet: zksync.Wallet) {
    const provider = zksWallet.provider;
    const accountState = await zksWallet.getAccountState();
    for (const token in accountState.committed.balances) {
        const tokenSymbol = provider.tokenSet.resolveTokenSymbol(token);
        if (tokenSymbol === 'MLTT') {
            continue;
        }

        const tokenCommittedBalance = BigNumber.from(accountState.committed.balances[token]);

        let withdrawFee = ethers.utils.parseEther('0.0');
        try {
            withdrawFee = (await provider.getTransactionFee('Withdraw', zksWallet.address(), token)).totalFee;
        } catch (e) {
            console.log(`Can't withdraw token ${tokenSymbol}: ${e}`);
            continue;
        }

        if (isOperationFeeAcceptable(tokenCommittedBalance, withdrawFee, MAX_LIQUIDATION_FEE_PERCENT)) {
            const amountAfterWithdraw = tokenCommittedBalance.sub(withdrawFee);
            console.log(
                `Withdrawing token, amount after withdraw: ${fmtToken(
                    provider,
                    token,
                    amountAfterWithdraw
                )}, fee: ${fmtToken(provider, token, withdrawFee)}`
            );
            const transaction = await zksWallet.withdrawFromSyncToEthereum({
                ethAddress: zksWallet.address(),
                token,
                amount: amountAfterWithdraw,
                fee: withdrawFee
            });
            console.log(`Tx hash: ${transaction.txHash}`);
            await transaction.awaitReceipt();

            await sendNotification(
                `Withdrawn ${await fmtTokenWithETHValue(provider, token, amountAfterWithdraw)}, tx hash: ${
                    transaction.txHash
                }`,
                NOTIFICATION_WEBHOOK_URL
            );
        }
    }
}

/** Transfer established tokens through ZkSync to the accumulator account */
/** Only tokens from the `establishedTokens` list should be transferred */
async function transferEstablishedTokens(zksWallet: zksync.Wallet, establishedTokens, feeAccumulatorAddress: string) {
    const provider = zksWallet.provider;
    const accountState = await zksWallet.getAccountState();
    for (const token in accountState.committed.balances) {
        if (!establishedTokens.includes(provider.tokenSet.resolveTokenSymbol(token))) {
            continue;
        }

        const tokenCommittedBalance = BigNumber.from(accountState.committed.balances[token]);

        const transferFee = (await provider.getTransactionFee('Transfer', feeAccumulatorAddress, token)).totalFee;

        if (isOperationFeeAcceptable(tokenCommittedBalance, transferFee, MAX_LIQUIDATION_FEE_PERCENT)) {
            const amountToTransfer = tokenCommittedBalance.sub(transferFee);
            console.log(
                `Transferring token, amount to transfer: ${fmtToken(
                    provider,
                    token,
                    amountToTransfer
                )}, fee: ${fmtToken(provider, token, transferFee)}`
            );
            const transaction = await zksWallet.syncTransfer({
                to: feeAccumulatorAddress,
                token,
                amount: amountToTransfer,
                fee: transferFee
            });
            console.log(`Tx hash: ${transaction.txHash}`);
            await transaction.awaitReceipt();

            await sendNotification(
                `Transfer ${await fmtTokenWithETHValue(
                    provider,
                    token,
                    amountToTransfer
                )}, accumulator address: ${feeAccumulatorAddress}, tx hash: ${transaction.txHash}`,
                NOTIFICATION_WEBHOOK_URL
            );
        }
    }
}

/** Swap tokens for ETH */
async function sellTokens(zksWallet: zksync.Wallet, ethParameters: EthParameters) {
    const zksProvider = zksWallet.provider;
    const tokens = await zksProvider.getTokens();
    for (const token in tokens) {
        const tokenSymbol = zksWallet.provider.tokenSet.resolveTokenSymbol(token);
        if (tokenSymbol === 'MLTT' || zksync.utils.isTokenETH(token)) {
            continue;
        }

        const tokenAmount = await zksWallet.getEthereumBalance(token);
        if (tokenAmount.eq(0)) {
            continue;
        }

        const req1inch =
            'https://api.1inch.exchange/v1.1/swapQuote?' +
            `fromTokenSymbol=${zksProvider.tokenSet.resolveTokenSymbol(token)}` +
            `&toTokenSymbol=ETH` +
            `&amount=${tokenAmount.toString()}` +
            `&slippage=${MAX_LIQUIDATION_FEE_SLIPPAGE}` +
            '&disableEstimate=true' +
            `&fromAddress=${zksWallet.address()}`;
        try {
            const expectedETHAfterTrade = await getExpectedETHSwapResult(
                tokens[token].symbol,
                tokens[token].decimals,
                tokenAmount
            );

            const apiResponse = await Axios.get(req1inch).then((resp) => resp.data);
            const approximateTxFee = BigNumber.from('300000').mul(apiResponse.gasPrice);
            const estimatedAmountAfterTrade = apiResponse.toTokenAmount;
            console.log(
                `Estimated swap result tokenAmount: ${fmtToken(
                    zksProvider,
                    token,
                    tokenAmount
                )} resultAmount: ${fmtToken(zksProvider, 'ETH', estimatedAmountAfterTrade)}, tx fee: ${fmtToken(
                    zksProvider,
                    'ETH',
                    approximateTxFee
                )}, coinGecko: ${fmtToken(zksProvider, 'ETH', estimatedAmountAfterTrade)}`
            );

            if (approximateTxFee.gte(estimatedAmountAfterTrade)) {
                continue;
            }

            // Crosscheck 1inch trade result with CoinGecko prices
            if (
                !isOperationFeeAcceptable(
                    expectedETHAfterTrade,
                    expectedETHAfterTrade.sub(estimatedAmountAfterTrade).abs(),
                    MAX_LIQUIDATION_FEE_SLIPPAGE
                )
            ) {
                console.log('1inch price is different then CoinGecko price');
                continue;
            }

            if (isOperationFeeAcceptable(estimatedAmountAfterTrade, approximateTxFee, MAX_LIQUIDATION_FEE_PERCENT)) {
                await approveTokenIfNotApproved(
                    zksWallet.ethSigner,
                    zksProvider.tokenSet.resolveTokenAddress(token),
                    INCH_APPROVE,
                    ethParameters
                );
                if (apiResponse.to.toLowerCase() != INCH_EXCHANGE.toLowerCase()) {
                    throw new Error('Incorrect exchange address');
                }

                console.log('Sending swap tx.');
                const ethTransaction = await zksWallet.ethSigner.sendTransaction({
                    from: apiResponse.from,
                    to: apiResponse.to,
                    gasLimit: BigNumber.from(apiResponse.gas),
                    gasPrice: BigNumber.from(apiResponse.gasPrice),
                    value: BigNumber.from(apiResponse.value),
                    data: apiResponse.data,
                    nonce: ethParameters.getNextNonce()
                });
                console.log(`Tx hash: ${ethTransaction.hash}`);

                await sendNotification(
                    `Swap ${await fmtTokenWithETHValue(zksProvider, token, tokenAmount)}, tx hash: ${
                        ethTransaction.hash
                    }`,
                    NOTIFICATION_WEBHOOK_URL
                );
            }
        } catch (err) {
            console.log(err);
            const response = err.response;
            console.log(
                `API error, status: ${response?.status} status: ${response?.statusText}, data.message: ${response?.data.message}`
            );
        }
    }
}

/** Send ETH to the accumulator account */
async function sendETH(zksWallet: zksync.Wallet, feeAccumulatorAddress: string, ethParameters: EthParameters) {
    const ethWallet = zksWallet.ethSigner;
    const ethProvider = ethWallet.provider;
    const ethBalance = await ethWallet.getBalance();
    const gasPrice = await ethProvider.getGasPrice();
    const ethTransferFee = BigNumber.from('21000').mul(gasPrice);
    if (ethBalance.gt(ETH_TRANSFER_THRESHOLD.add(ethTransferFee))) {
        const ethToSend = ethBalance.sub(ETH_TRANSFER_THRESHOLD.add(ethTransferFee));
        if (isOperationFeeAcceptable(ethToSend, ethTransferFee, MAX_LIQUIDATION_FEE_PERCENT)) {
            console.log(`Sending ${fmtToken(zksWallet.provider, 'ETH', ethToSend)} to ${feeAccumulatorAddress}`);
            const tx = await ethWallet.sendTransaction({
                to: feeAccumulatorAddress,
                value: ethToSend,
                gasPrice,
                nonce: ethParameters.getNextNonce()
            });
            console.log(`Tx hash: ${tx.hash}`);

            await sendNotification(
                `Send ${fmtToken(
                    zksWallet.provider,
                    'ETH',
                    ethToSend
                )}, accumulator address: ${feeAccumulatorAddress}, tx hash: ${tx.hash}`,
                NOTIFICATION_WEBHOOK_URL
            );
        }
    }
}

(async () => {
    const ethProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
    const ethWallet = new ethers.Wallet(FEE_ACCOUNT_PRIVATE_KEY).connect(ethProvider);
    const zksProvider = await zksync.getDefaultProvider(ETH_NETWORK, 'HTTP');
    const zksWallet = await zksync.Wallet.fromEthSigner(ethWallet, zksProvider);
    const ethParameters = new EthParameters(await zksWallet.ethSigner.getTransactionCount('latest'));
    try {
        if (!(await zksWallet.isSigningKeySet())) {
            console.log('Changing fee account signing key');
            const signingKeyTx = await zksWallet.setSigningKey({ feeToken: 'ETH', ethAuthType: 'ECDSA' });
            await signingKeyTx.awaitReceipt();
        }

        let operatorBalance = await ethProvider.getBalance(OPERATOR_FEE_ETH_ADDRESS);
        let feeAccountBalance = await ethProvider.getBalance(ethWallet.address);

        // If the operator has enough funds for work
        // and fee account have at least half of his threshold
        // use reserve fee accumulator
        if (
            operatorBalance.gte(THRESHOLD_AMOUNT_TO_USE_RESERVE_ADDRESS) &&
            feeAccountBalance.gte(ETH_TRANSFER_THRESHOLD.div(2))
        ) {
            // special scenario: send assets to the reserve fee accumulator
            console.log('All funds to be sent to the reserve fee accumulator address');

            console.log('Step 1 - transferring established tokens through ZkSync');
            await transferEstablishedTokens(
                zksWallet,
                ESTABLISHED_ASSETS_FOR_WITHDRAWING_THROUGH_ZKSYNC,
                RESERVE_FEE_ACCUMULATOR_ADDRESS
            );

            console.log('Step 2 - withdrawing tokens from ZkSync');
            await withdrawTokens(zksWallet);

            console.log('Step 3 - selling tokens for ETH');
            await sellTokens(zksWallet, ethParameters);

            console.log('Step 4 - sending ETH to the reserve fee accumulator address');
            await sendETH(zksWallet, RESERVE_FEE_ACCUMULATOR_ADDRESS, ethParameters);
        } else {
            // default scenario: all funds to be sent to the operator
            console.log('All funds to be sent to the operator address');

            console.log('Step 1 - withdrawing tokens from ZkSync');
            await withdrawTokens(zksWallet);

            console.log('Step 2 - selling tokens for ETH');
            await sellTokens(zksWallet, ethParameters);

            console.log('Step 3 - sending ETH to the operator address');
            await sendETH(zksWallet, OPERATOR_FEE_ETH_ADDRESS, ethParameters);
        }
    } catch (e) {
        console.error('Failed to proceed with fee liquidation: ', e);
        process.exit(1);
    } finally {
        await zksProvider.disconnect();
        process.exit(0);
    }
})();
