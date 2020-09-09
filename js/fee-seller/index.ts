/**
 * Script that sells accumulated transaction fees for ETH using 1inch exchange and transfer ETH to other account.
 *
 * Selling is done in steps:
 *    Step 1 - token is withdrawn to the ETH account
 *    Step 2 - token is swapped for ETH using 1inch
 *    Step 3 - ETH is transferred to the FEE_ACCUMULATOR_ADDRESS
 *
 *    Each steps happens one after another without waiting for previous to complete
 *    so this script should be run at a frequent intervals.
 *
 * Each operation is performed only if transaction fee of this operation is less then MAX_LIQUIDATION_FEE_PERCENT.
 *
 * See Env parameters for available configuration parameters
 */
import Axios from "axios";
import {BigNumber, ethers} from "ethers";
import * as zksync from "zksync";
import {approveTokenIfNotApproved, getExpectedETHSwapResult, isOperationFeeAcceptable} from "./utils";

/** Env parameters. */
const FEE_ACCOUNT_PRIVATE_KEY = process.env.FEE_ACCOUNT_PRIVATE_KEY;
const MAX_LIQUIDATION_FEE_PERCENT = parseInt(process.env.MAX_LIQUIDATION_FEE_PERCENT);
const FEE_ACCUMULATOR_ADDRESS = process.env.FEE_ACCUMULATOR_ADDRESS;
const ETH_NETWORK = process.env.ETH_NETWORK as any;
const WEB3_URL = process.env.WEB3_URL;
const MAX_LIQUIDATION_FEE_SLIPPAGE = parseInt(process.env.MAX_LIQUIDATION_FEE_SLIPPAGE) || 5;
/** Amount of ETH that should be left on the fee account after third step. */
const ETH_TRANSFER_THRESHOLD = process.env.ETH_TRANSFER_THRESHOLD ?
    ethers.utils.parseEther(process.env.ETH_TRANSFER_THRESHOLD) : ethers.utils.parseEther("3.0");


const INCH_APPROVE = "0xe4c9194962532feb467dce8b3d42419641c6ed2e";
const INCH_EXCHANGE = "0x11111254369792b2Ca5d084aB5eEA397cA8fa48B";

(async () => {
    const ethProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
    const ethWallet = new ethers.Wallet(FEE_ACCOUNT_PRIVATE_KEY).connect(ethProvider);
    const zksProvider = await zksync.getDefaultProvider(ETH_NETWORK, "HTTP");
    const fmtToken = (token, amount) => `${zksProvider.tokenSet.formatToken(token, amount)} ${zksProvider.tokenSet.resolveTokenSymbol(token)}`;
    const zksWallet = await zksync.Wallet.fromEthSigner(ethWallet, zksProvider);
    try {
        if (!await zksWallet.isSigningKeySet()) {
            console.log("Changing fee account signing key");
            const signingKeyTx = await zksWallet.setSigningKey();
            await signingKeyTx.awaitReceipt();
        }

        console.log("Step 1 - withdrawing tokens");
        // Step 1 withdraw everything that has to be withdrawn
        const accountState = await zksWallet.getAccountState();
        for (const token in accountState.committed.balances) {
            if (zksProvider.tokenSet.resolveTokenSymbol(token) === "MLTT") {
                continue;
            }

            const tokenBalance = BigNumber.from(accountState.committed.balances[token]);
            const withdrawFee = (await zksProvider.getTransactionFee("Withdraw", zksWallet.address(), token)).totalFee;

            if (isOperationFeeAcceptable(tokenBalance, withdrawFee, MAX_LIQUIDATION_FEE_PERCENT)) {
                const amountAfterWithdraw = tokenBalance.sub(withdrawFee);
                console.log(`Withdrawing token: ${token}, amount after withdraw: ${zksProvider.tokenSet.formatToken(token, amountAfterWithdraw)}, fee: ${zksProvider.tokenSet.formatToken(token, withdrawFee)}`);
                const transaction = await zksWallet.withdrawFromSyncToEthereum({
                    ethAddress: zksWallet.address(),
                    token,
                    amount: amountAfterWithdraw,
                    fee: withdrawFee,
                });
                console.log(`Tx hash: ${transaction.txHash}`);
                await transaction.awaitReceipt();
            }
        }

        // Step 2 sell onchain balance tokens
        console.log("Step 2 - selling tokens");
        const tokens = await zksProvider.getTokens();
        for (const token in tokens) {
            if (zksProvider.tokenSet.resolveTokenSymbol(token) === "MLTT" || zksync.utils.isTokenETH(token)) {
                continue;
            }

            const tokenAmount = await zksWallet.getEthereumBalance(token);
            if (tokenAmount.eq(0)) {
                continue;
            }


            const req1inch = "https://api.1inch.exchange/v1.1/swapQuote?" +
                `fromTokenSymbol=${zksProvider.tokenSet.resolveTokenSymbol(token)}` +
                `&toTokenSymbol=ETH` +
                `&amount=${tokenAmount.toString()}` +
                `&slippage=${MAX_LIQUIDATION_FEE_SLIPPAGE}` +
                "&disableEstimate=true" +
                `&fromAddress=${zksWallet.address()}`;
            try {
                const expectedETHAfterTrade = await getExpectedETHSwapResult(tokens[token].symbol, tokens[token].decimals, tokenAmount);

                const apiResponse = await Axios.get(req1inch).then((resp) => resp.data);
                const approximateTxFee = BigNumber.from("300000").mul(apiResponse.gasPrice);
                const estimatedAmountAfterTrade = apiResponse.toTokenAmount;
                console.log(`Estimated swap result tokenAmount: ${fmtToken(token, tokenAmount)} resultAmount: ${fmtToken("ETH", estimatedAmountAfterTrade)}, tx fee: ${fmtToken("ETH", approximateTxFee)}, coinGecko: ${fmtToken("ETH", estimatedAmountAfterTrade)}`);

                if (approximateTxFee.gte(estimatedAmountAfterTrade)) {
                    continue;
                }

                // Crosscheck 1inch trade result with CoinGecko prices
                if (!isOperationFeeAcceptable(expectedETHAfterTrade, expectedETHAfterTrade.sub(estimatedAmountAfterTrade).abs(), MAX_LIQUIDATION_FEE_SLIPPAGE)) {
                    console.log("1inch price is different then CoinGecko price");
                    continue
                }

                if (isOperationFeeAcceptable(estimatedAmountAfterTrade, approximateTxFee, MAX_LIQUIDATION_FEE_PERCENT)) {
                    await approveTokenIfNotApproved(ethWallet, zksProvider.tokenSet.resolveTokenAddress(token), INCH_APPROVE)
                    if (apiResponse.to.toLowerCase() != INCH_EXCHANGE.toLowerCase()) {
                        throw new Error("Incorrect exchange address");
                    }

                    console.log("Sending swap tx.");
                    const ethTransaction = await ethWallet.sendTransaction({
                        from: apiResponse.from,
                        to: apiResponse.to,
                        gasLimit: BigNumber.from(apiResponse.gas),
                        gasPrice: BigNumber.from(apiResponse.gasPrice),
                        value: BigNumber.from(apiResponse.value),
                        data: apiResponse.data,
                    });
                    console.log(`Tx hash: ${ethTransaction.hash}`);
                }
            } catch (err) {
                console.log(err)
                const response = err.response;
                console.log(`API error, status: ${response?.status} status: ${response?.statusText}, data.message: ${response?.data.message}`);
            }
        }

        // Step 3 - moving Ethereum to the operator account
        console.log("Step 2 - sending ETH");
        const ethBalance = await ethWallet.getBalance();
        if (ethBalance.gt(ETH_TRANSFER_THRESHOLD)) {
            const ethTransferFee = BigNumber.from("21000").mul(await ethProvider.getGasPrice());
            const ethToSend = ethBalance.sub(ETH_TRANSFER_THRESHOLD);
            if (isOperationFeeAcceptable(ethToSend, ethTransferFee, MAX_LIQUIDATION_FEE_PERCENT)) {
                console.log(`Sending ${fmtToken("ETH", ethToSend)} to ${FEE_ACCUMULATOR_ADDRESS}`);
                const tx = await ethWallet.sendTransaction({to: FEE_ACCUMULATOR_ADDRESS, value: ethToSend});
                console.log(`Tx hash: ${tx.hash}`);
            }
        }
    } catch (e) {
        console.error("Failed to proceed with fee liquidation: ", e);
        process.exit(1);
    } finally {
        await zksProvider.disconnect();
        process.exit(0);
    }
})();
