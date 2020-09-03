import { Wallet, Provider, ETHProxy, types, utils as zkutils } from "zksync";
// HACK: using require as type system work-around
const franklin_abi = require("../../contracts/build/ZkSync.json").abi;
import { ethers, Contract, BigNumber, BigNumberish, utils } from "ethers";
import { IERC20_INTERFACE, sleep } from "zksync/src/utils";
import * as apitype from "./api-type-validate";
import * as assert from "assert";

const WEB3_URL = process.env.WEB3_URL;
const VERIFY_TIMEOUT = 120000; // 2 minutes in ms.

const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";
console.log("Running integration test on the ", network, " network");
const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
if (network == "localhost") {
    ethersProvider.pollingInterval = 100;
}

let syncProvider: Provider;

async function getOperatorBalance(token: types.TokenLike, type: "committed" | "verified" = "committed") {
    const accountState = await syncProvider.getState(process.env.OPERATOR_FEE_ETH_ADDRESS);
    const tokenSet = syncProvider.tokenSet;
    const tokenSymbol = tokenSet.resolveTokenSymbol(token);
    let balance;
    if (type == "committed") {
        balance = accountState.committed.balances[tokenSymbol] || "0";
    } else {
        balance = accountState.verified.balances[tokenSymbol] || "0";
    }
    return BigNumber.from(balance);
}

async function testAutoApprovedDeposit(
    depositWallet: Wallet,
    syncWallet: Wallet,
    token: types.TokenLike,
    amount: BigNumberish
) {
    const balanceBeforeDep = await syncWallet.getBalance(token);

    const startTime = new Date().getTime();
    const depositHandle = await depositWallet.depositToSyncFromEthereum({
        depositTo: syncWallet.address(),
        token: token,
        amount,
        approveDepositAmountForERC20: true,
    });
    console.log(`Deposit posted: ${new Date().getTime() - startTime} ms`);

    await depositHandle.awaitReceipt();
    console.log(`Deposit committed: ${new Date().getTime() - startTime} ms`);
    const balanceAfterDep = await syncWallet.getBalance(token);

    if (!balanceAfterDep.sub(balanceBeforeDep).eq(amount)) {
        throw new Error("Deposit checks failed");
    }
}

async function testDeposit(depositWallet: Wallet, syncWallet: Wallet, token: types.TokenLike, amount: BigNumber) {
    const balanceBeforeDep = await syncWallet.getBalance(token);

    const startTime = new Date().getTime();
    if (!zkutils.isTokenETH(token)) {
        if (await depositWallet.isERC20DepositsApproved(token)) {
            throw new Error("Token should not be approved");
        }
        const approveERC20 = await depositWallet.approveERC20TokenDeposits(token);
        await approveERC20.wait();
        console.log(`Deposit approved: ${new Date().getTime() - startTime} ms`);
        if (!(await depositWallet.isERC20DepositsApproved(token))) {
            throw new Error("Token be approved");
        }
    }
    const depositHandle = await depositWallet.depositToSyncFromEthereum({
        depositTo: syncWallet.address(),
        token: token,
        amount,
    });
    console.log(`Deposit posted: ${new Date().getTime() - startTime} ms`);
    await depositHandle.awaitReceipt();
    console.log(`Deposit committed: ${new Date().getTime() - startTime} ms`);
    const balanceAfterDep = await syncWallet.getBalance(token);

    if (!zkutils.isTokenETH(token)) {
        if (!(await depositWallet.isERC20DepositsApproved(token))) {
            throw new Error("Token should still be approved");
        }
    }

    if (!balanceAfterDep.sub(balanceBeforeDep).eq(amount)) {
        throw new Error("Deposit checks failed");
    }
}

async function testTransferToSelf(syncWallet: Wallet, token: types.TokenLike, amount: BigNumber) {
    const fullFee = await syncProvider.getTransactionFee("Transfer", syncWallet.address(), token);
    const fee = fullFee.totalFee;

    const walletBeforeTransfer = await syncWallet.getBalance(token);
    const operatorBeforeTransfer = await getOperatorBalance(token);
    const startTime = new Date().getTime();
    const transferToNewHandle = await syncWallet.syncTransfer({
        to: syncWallet.address(),
        token,
        amount,
        fee,
    });
    console.log(`Transfer to self posted: ${new Date().getTime() - startTime} ms`);
    await transferToNewHandle.awaitReceipt();
    console.log(`Transfer to self committed: ${new Date().getTime() - startTime} ms`);
    const walletAfterTransfer = await syncWallet.getBalance(token);
    const operatorAfterTransfer = await getOperatorBalance(token);

    let transferCorrect = true;
    transferCorrect = transferCorrect && walletBeforeTransfer.sub(fee).eq(walletAfterTransfer);
    transferCorrect = transferCorrect && operatorAfterTransfer.sub(operatorBeforeTransfer).eq(fee);
    if (!transferCorrect) {
        throw new Error("Transfer to self checks failed");
    }
}

async function testTransfer(
    syncWallet1: Wallet,
    syncWallet2: Wallet,
    token: types.TokenLike,
    amount: BigNumber,
    timeoutBeforeReceipt = 0
) {
    const fullFee = await syncProvider.getTransactionFee("Transfer", syncWallet2.address(), token);
    const fee = fullFee.totalFee;

    const wallet1BeforeTransfer = await syncWallet1.getBalance(token);
    const wallet2BeforeTransfer = await syncWallet2.getBalance(token);
    const operatorBeforeTransfer = await getOperatorBalance(token);
    const startTime = new Date().getTime();
    const transferToNewHandle = await syncWallet1.syncTransfer({
        to: syncWallet2.address(),
        token,
        amount,
        fee,
    });
    console.log(`Transfer posted: ${new Date().getTime() - startTime} ms`);
    await sleep(timeoutBeforeReceipt);
    await transferToNewHandle.awaitReceipt();
    console.log(`Transfer committed: ${new Date().getTime() - startTime} ms`);
    const wallet1AfterTransfer = await syncWallet1.getBalance(token);
    const wallet2AfterTransfer = await syncWallet2.getBalance(token);
    const operatorAfterTransfer = await getOperatorBalance(token);

    let transferCorrect = true;
    transferCorrect = transferCorrect && wallet1BeforeTransfer.sub(wallet1AfterTransfer).eq(amount.add(fee));
    transferCorrect = transferCorrect && wallet2AfterTransfer.sub(wallet2BeforeTransfer).eq(amount);
    transferCorrect = transferCorrect && operatorAfterTransfer.sub(operatorBeforeTransfer).eq(fee);
    if (!transferCorrect) {
        throw new Error("Transfer checks failed");
    }
}

async function testWithdraw(
    contract: Contract,
    withdrawTo: Wallet,
    syncWallet: Wallet,
    token: types.TokenLike,
    amount: BigNumber
) {
    const fullFee = await syncProvider.getTransactionFee("Withdraw", withdrawTo.address(), token);
    const fee = fullFee.totalFee;

    const wallet2BeforeWithdraw = await syncWallet.getBalance(token);
    const operatorBeforeWithdraw = await getOperatorBalance(token);
    const onchainBalanceBeforeWithdraw = await withdrawTo.getEthereumBalance(token);
    const startTime = new Date().getTime();
    const withdrawHandle = await syncWallet.withdrawFromSyncToEthereum({
        ethAddress: withdrawTo.address(),
        token,
        amount,
        fee,
    });
    console.log(`Withdraw posted: ${new Date().getTime() - startTime} ms`);

    // Await for verification with a timeout set.
    await promiseTimeout(VERIFY_TIMEOUT, withdrawHandle.awaitVerifyReceipt());
    console.log(`Withdraw verified: ${new Date().getTime() - startTime} ms`);
    const wallet2AfterWithdraw = await syncWallet.getBalance(token);
    const operatorAfterWithdraw = await getOperatorBalance(token);
    const onchainBalanceAfterWithdraw = await withdrawTo.getEthereumBalance(token);

    const tokenId = await withdrawTo.provider.tokenSet.resolveTokenId(token);
    const pendingToBeOnchainBalance = await contract.getBalanceToWithdraw(await withdrawTo.address(), tokenId);

    if (!wallet2BeforeWithdraw.sub(wallet2AfterWithdraw).eq(amount.add(fee))) {
        throw new Error("Wrong amount on wallet after WITHDRAW");
    }
    if (!operatorAfterWithdraw.sub(operatorBeforeWithdraw).eq(fee)) {
        throw new Error("Wrong amount of operator fees after WITHDRAW");
    }
    if (!onchainBalanceAfterWithdraw.add(pendingToBeOnchainBalance).sub(onchainBalanceBeforeWithdraw).eq(amount)) {
        throw new Error("Wrong amount onchain after WITHDRAW");
    }
}

async function testFastWithdraw(
    contract: Contract,
    withdrawTo: Wallet,
    syncWallet: Wallet,
    token: types.TokenLike,
    amount: BigNumber
) {
    const fullFee = await syncProvider.getTransactionFee("FastWithdraw", withdrawTo.address(), token);
    const fee = fullFee.totalFee;

    const wallet2BeforeWithdraw = await syncWallet.getBalance(token);
    const operatorBeforeWithdraw = await getOperatorBalance(token);
    const onchainBalanceBeforeWithdraw = await withdrawTo.getEthereumBalance(token);
    const startTime = new Date().getTime();
    const withdrawHandle = await syncWallet.withdrawFromSyncToEthereum({
        ethAddress: withdrawTo.address(),
        token,
        amount,
        fee,
        fastProcessing: true,
    });
    console.log(`Fast withdraw posted: ${new Date().getTime() - startTime} ms`);

    // Await for verification with a timeout set.
    await promiseTimeout(VERIFY_TIMEOUT, withdrawHandle.awaitVerifyReceipt());
    console.log(`Fast withdraw verified: ${new Date().getTime() - startTime} ms`);
    const wallet2AfterWithdraw = await syncWallet.getBalance(token);
    const operatorAfterWithdraw = await getOperatorBalance(token);
    const onchainBalanceAfterWithdraw = await withdrawTo.getEthereumBalance(token);

    const tokenId = await withdrawTo.provider.tokenSet.resolveTokenId(token);
    const pendingToBeOnchainBalance = await contract.getBalanceToWithdraw(await withdrawTo.address(), tokenId);

    if (!wallet2BeforeWithdraw.sub(wallet2AfterWithdraw).eq(amount.add(fee))) {
        throw new Error("Wrong amount on wallet after fast withdraw");
    }
    if (!operatorAfterWithdraw.sub(operatorBeforeWithdraw).eq(fee)) {
        throw new Error("Wrong amount of operator fees after fast withdraw");
    }
    if (!onchainBalanceAfterWithdraw.add(pendingToBeOnchainBalance).sub(onchainBalanceBeforeWithdraw).eq(amount)) {
        throw new Error("Wrong amount onchain after fast withdraw");
    }
}

async function testChangePubkeyOnchain(syncWallet: Wallet) {
    if (!(await syncWallet.isSigningKeySet())) {
        const startTime = new Date().getTime();
        await (await syncWallet.onchainAuthSigningKey("committed")).wait();
        const changePubkeyHandle = await syncWallet.setSigningKey("committed", true);
        console.log(`Change pubkey onchain posted: ${new Date().getTime() - startTime} ms`);
        await changePubkeyHandle.awaitReceipt();
        console.log(`Change pubkey onchain committed: ${new Date().getTime() - startTime} ms`);
        if (!(await syncWallet.isSigningKeySet())) {
            throw new Error("Change pubkey onchain failed");
        }
    }
}

async function testChangePubkeyOffchain(syncWallet: Wallet) {
    if (!(await syncWallet.isSigningKeySet())) {
        const startTime = new Date().getTime();
        const changePubkeyHandle = await syncWallet.setSigningKey();
        console.log(`Change pubkey offchain posted: ${new Date().getTime() - startTime} ms`);
        await changePubkeyHandle.awaitReceipt();
        console.log(`Change pubkey offchain committed: ${new Date().getTime() - startTime} ms`);
        if (!(await syncWallet.isSigningKeySet())) {
            throw new Error("Change pubkey offchain failed");
        }
    }
}

async function testThrowingErrorOnTxFail(zksyncDepositorWallet: Wallet) {
    console.log("testThrowingErrorOnTxFail");
    let testPassed = true;

    const ethWallet = ethers.Wallet.createRandom().connect(ethersProvider);
    const syncWallet = await Wallet.fromEthSigner(ethWallet, syncProvider);

    // Create account so transfer would fail while tx is being executed
    const initialDeposit = await zksyncDepositorWallet.depositToSyncFromEthereum({
        depositTo: ethWallet.address,
        token: "ETH",
        amount: "1",
    });
    await initialDeposit.awaitReceipt();

    try {
        const tx = await syncWallet.syncTransfer({
            to: zksyncDepositorWallet.address(),
            token: "ETH",
            amount: utils.parseEther("0.01"),
            fee: utils.parseEther("10"),
        });
        await tx.awaitVerifyReceipt();
        testPassed = false;
    } catch (e) {
        console.log("Error (expected) on sync tx fail:", e);
    }

    if (!testPassed) {
        throw new Error("testThrowingErrorOnTxFail failed");
    }
    console.log("Test ok");
}

async function moveFunds(
    contract: Contract,
    ethProxy: ETHProxy,
    depositWallet: Wallet,
    syncWallet1: Wallet,
    syncWallet2: Wallet,
    token: types.TokenLike,
    depositAmountETH: string
) {
    const depositAmount = utils.parseEther(depositAmountETH);

    // we do two transfers to test transfer to new and ordinary transfer.
    const transfersAmount = depositAmount.div(10);
    const withdrawAmount = transfersAmount.div(10);

    await testAutoApprovedDeposit(depositWallet, syncWallet1, token, depositAmount.div(2));
    console.log(`Auto approved deposit ok, Token: ${token}`);
    await testDeposit(depositWallet, syncWallet1, token, depositAmount.div(2));
    console.log(`Forever approved deposit ok, Token: ${token}`);
    await testChangePubkeyOnchain(syncWallet1);
    console.log(`Change pubkey onchain ok`);
    await testTransfer(syncWallet1, syncWallet2, token, transfersAmount);
    console.log(`Transfer to new ok, Token: ${token}`);
    await testTransfer(syncWallet1, syncWallet2, token, transfersAmount);
    console.log(`Transfer ok, Token: ${token}`);
    await testTransferToSelf(syncWallet1, token, transfersAmount);
    console.log(`Transfer to self with fee ok, Token: ${token}`);
    await testChangePubkeyOffchain(syncWallet2);
    console.log(`Change pubkey offchain ok`);
    await testSendingWithWrongSignature(syncWallet1, syncWallet2);
    await testWithdraw(contract, syncWallet2, syncWallet2, token, withdrawAmount);
    console.log(`Withdraw ok, Token: ${token}`);
    // Note that wallet is different from `testWithdraw` to not interfere with previous withdrawal.
    await testFastWithdraw(contract, syncWallet1, syncWallet1, token, withdrawAmount);
    console.log(`Fast withdraw ok, Token: ${token}`);
}

async function testSendingWithWrongSignature(syncWallet1: Wallet, syncWallet2: Wallet) {
    const signedTransfer: types.Transfer = syncWallet1.signer.signSyncTransfer({
        accountId: await syncWallet1.getAccountId(),
        from: syncWallet1.address(),
        to: syncWallet2.address(),
        tokenId: 0,
        amount: utils.parseEther("0.001"),
        fee: utils.parseEther("0.001"),
        nonce: await syncWallet1.getNonce(),
    });

    const ETH_SIGNATURE_LENGTH_PREFIXED = 132;
    const fakeEthSignature: types.TxEthSignature = {
        signature: "0x".padEnd(ETH_SIGNATURE_LENGTH_PREFIXED, "0"),
        type: "EthereumSignature",
    };

    try {
        await syncWallet1.provider.submitTx(signedTransfer, fakeEthSignature);
        assert(false, "sending tx with incorrect eth signature must throw");
    } catch (e) {
        assert(
            e.jrpcError.message == "Eth signature is incorrect",
            "sending tx with incorrect eth signature must fail"
        );
    }

    const fullFee = await syncProvider.getTransactionFee("Withdraw", syncWallet1.address(), "ETH");
    const fee = fullFee.totalFee;

    const signedWithdraw = syncWallet1.signer.signSyncWithdraw({
        accountId: await syncWallet1.getAccountId(),
        from: syncWallet1.address(),
        ethAddress: syncWallet1.address(),
        tokenId: 0,
        amount: utils.parseEther("0.001"),
        fee: fee,
        nonce: await syncWallet1.getNonce(),
    });

    try {
        await syncWallet1.provider.submitTx(signedWithdraw, fakeEthSignature);
        assert(false, "sending tx with incorrect eth signature must throw");
    } catch (e) {
        assert(
            e.jrpcError.message == "Eth signature is incorrect",
            `sending tx with incorrect eth signature must fail, got message: ${e.jrpcError.message}`
        );
    }
}

function promiseTimeout(ms, promise) {
    // Create a promise that rejects in <ms> milliseconds
    let timeout = new Promise((resolve, reject) => {
        let id = setTimeout(() => {
            clearTimeout(id);
            reject("Timed out in " + ms + "ms.");
        }, ms);
    });

    // Returns a race between our timeout and the passed in promise
    return Promise.race([promise, timeout]);
}

async function checkFailedTransactionResending(
    contract: Contract,
    depositWallet: Wallet,
    syncWallet1: Wallet,
    syncWallet2: Wallet
) {
    console.log("Checking invalid transaction resending");
    const amount = utils.parseEther("0.2");

    const fullFee = await syncProvider.getTransactionFee("Transfer", syncWallet2.address(), "ETH");
    const fee = fullFee.totalFee;

    await testAutoApprovedDeposit(depositWallet, syncWallet1, "ETH", amount.div(2).add(fee));
    await testChangePubkeyOnchain(syncWallet1);
    try {
        await testTransfer(syncWallet1, syncWallet2, "ETH", amount);
    } catch (e) {
        assert(e?.value?.failReason == `Not enough balance`);
        console.log("Transfer failed (expected)");
    }

    await testDeposit(depositWallet, syncWallet1, "ETH", amount.div(2));
    // We should wait some `timeoutBeforeReceipt` to give server enough time
    // to move our transaction with success flag from mempool to statekeeper
    //
    // If we won't wait enough, then we'll get the receipt for the previous, failed tx,
    // which has the same hash. The new (successful) receipt will be available only
    // when tx will be executed again in state keeper, so we must wait for it.
    await testTransfer(syncWallet1, syncWallet2, "ETH", amount, 3000);
}

(async () => {
    try {
        if (process.argv[2] === "http") {
            console.log("Testing with http provider");
            syncProvider = await Provider.newHttpProvider(process.env.HTTP_API_ADDR, 50);
        } else {
            console.log("Testing with websocket provider");
            syncProvider = await Provider.newWebsocketProvider(process.env.WS_API_ADDR);
        }
        const ERC20_SYMBOL = "DAI";
        const ERC20_ADDRESS = syncProvider.tokenSet.resolveTokenAddress(ERC20_SYMBOL);

        const ethProxy = new ETHProxy(ethersProvider, syncProvider.contractAddress);

        const ethWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(
            ethersProvider
        );
        const erc20 = new Contract(ERC20_ADDRESS, IERC20_INTERFACE, ethWallet);
        const syncDepositorWallet = ethers.Wallet.createRandom().connect(ethersProvider);
        await (
            await ethWallet.sendTransaction({ to: syncDepositorWallet.address, value: utils.parseEther("6.0") })
        ).wait();
        await (await erc20.transfer(syncDepositorWallet.address, utils.parseEther("210.0"))).wait();
        const zksyncDepositorWallet = await Wallet.fromEthSigner(syncDepositorWallet, syncProvider);

        const syncWalletSigner = ethers.Wallet.createRandom().connect(ethersProvider);
        await await ethWallet.sendTransaction({ to: syncWalletSigner.address, value: utils.parseEther("6.0") });
        const syncWallet = await Wallet.fromEthSigner(syncWalletSigner, syncProvider);

        const contract = new Contract(syncProvider.contractAddress.mainContract, franklin_abi, ethWallet);

        const ethWallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
        await await ethWallet.sendTransaction({ to: ethWallet2.address, value: utils.parseEther("6.0") });
        const syncWallet2 = await Wallet.fromEthSigner(ethWallet2, syncProvider);

        const ethWallet3 = ethers.Wallet.createRandom().connect(ethersProvider);
        await await ethWallet.sendTransaction({ to: ethWallet3.address, value: utils.parseEther("6.0") });
        const syncWallet3 = await Wallet.fromEthSigner(ethWallet3, syncProvider);

        await testThrowingErrorOnTxFail(zksyncDepositorWallet);

        apitype.deleteUnusedGenFiles();
        await apitype.checkStatusResponseType();
        await apitype.checkTestnetConfigResponseType();

        // Check that transaction can be successfully executed after previous failure.
        const ethWallet4 = ethers.Wallet.createRandom().connect(ethersProvider);
        await await ethWallet.sendTransaction({ to: ethWallet4.address, value: utils.parseEther("6.0") });
        const syncWallet4 = await Wallet.fromEthSigner(ethWallet4, syncProvider);
        const ethWallet5 = ethers.Wallet.createRandom().connect(ethersProvider);
        await await ethWallet.sendTransaction({ to: ethWallet5.address, value: utils.parseEther("6.0") });
        const syncWallet5 = await Wallet.fromEthSigner(ethWallet5, syncProvider);
        await checkFailedTransactionResending(contract, zksyncDepositorWallet, syncWallet4, syncWallet5);

        await moveFunds(contract, ethProxy, zksyncDepositorWallet, syncWallet, syncWallet2, ERC20_SYMBOL, "200.0");
        await moveFunds(contract, ethProxy, zksyncDepositorWallet, syncWallet, syncWallet3, "ETH", "1.0");

        await syncProvider.disconnect();
    } catch (e) {
        console.error("Error: ", e);
        process.exit(1);
    }
})();
