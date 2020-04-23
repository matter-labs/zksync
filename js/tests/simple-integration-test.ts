import {
    Wallet,
    Provider,
    ETHProxy, types, utils as zkutils
} from "zksync";
// HACK: using require as type system work-around
const franklin_abi = require('../../contracts/build/Franklin.json');
import {ethers, utils, Contract} from "ethers";
import {bigNumberify, parseEther} from "ethers/utils";
import {IERC20_INTERFACE} from "zksync/build/utils";


const WEB3_URL = process.env.WEB3_URL;
// Mnemonic for eth wallet.
const MNEMONIC = process.env.TEST_MNEMONIC;

const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";
console.log("Running integration test on the ", network, " network");
const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);

let syncProvider: Provider;

async function getOperatorBalance(token: types.TokenLike, type: "committed" | "verified" = "committed") {
    const accountState = await syncProvider.getState(process.env.OPERATOR_FRANKLIN_ADDRESS);
    const tokenSet = syncProvider.tokenSet;
    const tokenSymbol = tokenSet.resolveTokenSymbol(token);
    let balance;
    if (type == "committed") {
        balance = accountState.committed.balances[tokenSymbol] || "0";
    } else {
        balance = accountState.verified.balances[tokenSymbol] || "0";
    }
    return utils.bigNumberify(balance);
}

async function testAutoApprovedDeposit(depositWallet: Wallet, syncWallet: Wallet, token: types.TokenLike, amount: utils.BigNumberish) {
    const balanceBeforeDep = await syncWallet.getBalance(token);

    const startTime = new Date().getTime();
    const depositHandle = await depositWallet.depositToSyncFromEthereum(
        {
            depositTo: syncWallet.address(),
            token: token,
            amount,
            approveDepositAmountForERC20: true,
        });
    console.log(`Deposit posted: ${(new Date().getTime()) - startTime} ms`);
    await depositHandle.awaitReceipt();
    console.log(`Deposit committed: ${(new Date().getTime()) - startTime} ms`);
    const balanceAfterDep = await syncWallet.getBalance(token);

    if (!balanceAfterDep.sub(balanceBeforeDep).eq(amount)) {
        throw new Error("Deposit checks failed");
    }
}

async function testDeposit(depositWallet: Wallet, syncWallet: Wallet, token: types.TokenLike, amount: utils.BigNumberish) {
    const balanceBeforeDep = await syncWallet.getBalance(token);

    const startTime = new Date().getTime();
    if (!zkutils.isTokenETH(token)) {
        if (await depositWallet.isERC20DepositsApproved(token)){
            throw new Error("Token should not be approved");
        }
        const approveERC20 = await depositWallet.approveERC20TokenDeposits(token);
        await approveERC20.wait();
        console.log(`Deposit approved: ${(new Date().getTime()) - startTime} ms`);
        if (!await depositWallet.isERC20DepositsApproved(token)){
            throw new Error("Token be approved");
        }
    }
    const depositHandle = await depositWallet.depositToSyncFromEthereum(
        {
            depositTo: syncWallet.address(),
            token: token,
            amount,
        });
    console.log(`Deposit posted: ${(new Date().getTime()) - startTime} ms`);
    await depositHandle.awaitReceipt();
    console.log(`Deposit committed: ${(new Date().getTime()) - startTime} ms`);
    const balanceAfterDep = await syncWallet.getBalance(token);

    if (!zkutils.isTokenETH(token)) {
        if (!await depositWallet.isERC20DepositsApproved(token)){
            throw new Error("Token should still be approved");
        }
    }

    if (!balanceAfterDep.sub(balanceBeforeDep).eq(amount)) {
        throw new Error("Deposit checks failed");
    }
}

async function testTransferToSelf(syncWallet: Wallet, token: types.TokenLike, amount: utils.BigNumber, fee: utils.BigNumber) {
    const walletBeforeTransfer = await syncWallet.getBalance(token);
    const operatorBeforeTransfer = await getOperatorBalance(token);
    const startTime = new Date().getTime();
    const transferToNewHandle = await syncWallet.syncTransfer({
        to: syncWallet.address(),
        token,
        amount,
        fee
    });
    console.log(`Transfer to self posted: ${(new Date().getTime()) - startTime} ms`);
    await transferToNewHandle.awaitReceipt();
    console.log(`Transfer to self committed: ${(new Date().getTime()) - startTime} ms`);
    const walletAfterTransfer = await syncWallet.getBalance(token);
    const operatorAfterTransfer = await getOperatorBalance(token);

    let transferCorrect = true;
    transferCorrect = transferCorrect && walletBeforeTransfer.sub(fee).eq(walletAfterTransfer);
    transferCorrect = transferCorrect && operatorAfterTransfer.sub(operatorBeforeTransfer).eq(fee);
    if (!transferCorrect) {
        throw new Error("Transfer to self checks failed");
    }
}

async function testTransfer(syncWallet1: Wallet, syncWallet2: Wallet, token: types.TokenLike, amount: utils.BigNumber, fee: utils.BigNumber) {
    const wallet1BeforeTransfer = await syncWallet1.getBalance(token);
    const wallet2BeforeTransfer = await syncWallet2.getBalance(token);
    const operatorBeforeTransfer = await getOperatorBalance(token);
    const startTime = new Date().getTime();
    const transferToNewHandle = await syncWallet1.syncTransfer({
        to: syncWallet2.address(),
        token,
        amount,
        fee
    });
    console.log(`Transfer posted: ${(new Date().getTime()) - startTime} ms`);
    await transferToNewHandle.awaitReceipt();
    console.log(`Transfer committed: ${(new Date().getTime()) - startTime} ms`);
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

async function testWithdraw(contract: Contract, withdrawTo: Wallet, syncWallet: Wallet, token: types.TokenLike, amount: utils.BigNumber, fee: utils.BigNumber) {
    const wallet2BeforeWithdraw = await syncWallet.getBalance(token);
    const operatorBeforeWithdraw = await getOperatorBalance(token);
    const onchainBalanceBeforeWithdraw = await withdrawTo.getEthereumBalance(token);
    const startTime = new Date().getTime();
    const withdrawHandle = await syncWallet.withdrawFromSyncToEthereum({
        ethAddress: withdrawTo.address(),
        token,
        amount,
        fee
    });
    console.log(`Withdraw posted: ${(new Date().getTime()) - startTime} ms`);
    await withdrawHandle.awaitVerifyReceipt();
    console.log(`Withdraw verified: ${(new Date().getTime()) - startTime} ms`);
    const wallet2AfterWithdraw = await syncWallet.getBalance(token);
    const operatorAfterWithdraw = await getOperatorBalance(token);
    const onchainBalanceAfterWithdraw = await withdrawTo.getEthereumBalance(token);

    const tokenId = await withdrawTo.provider.tokenSet.resolveTokenId(token);
    const pendingToBeOnchainBalance = (await contract.balancesToWithdraw(
        await withdrawTo.address(),
        tokenId,
    )).balanceToWithdraw;

    if (!wallet2BeforeWithdraw.sub(wallet2AfterWithdraw).eq(amount.add(fee))) {
        throw new Error("Wrong amount on wallet after WITHDRAW");
    }
    if (!operatorAfterWithdraw.sub(operatorBeforeWithdraw).eq(fee)) {
        throw new Error("Wrong amount of operator fees after WITHDRAW");
    }
    if (!(onchainBalanceAfterWithdraw.add(pendingToBeOnchainBalance)).sub(onchainBalanceBeforeWithdraw).eq(amount)) {
        throw new Error("Wrong amount onchain after WITHDRAW");
    }
}

async function testChangePubkeyOnchain(syncWallet: Wallet) {
    if (! await syncWallet.isSigningKeySet()) {
        const startTime = new Date().getTime();
        await (await syncWallet.onchainAuthSigningKey("committed")).wait();
        const changePubkeyHandle = await syncWallet.setSigningKey("committed", true);
        console.log(`Change pubkey onchain posted: ${(new Date().getTime()) - startTime} ms`);
        await changePubkeyHandle.awaitReceipt();
        console.log(`Change pubkey onchain committed: ${(new Date().getTime()) - startTime} ms`);
        if (! await syncWallet.isSigningKeySet()) {
            throw new Error("Change pubkey onchain failed");
        }
    }
}

async function testChangePubkeyOffchain(syncWallet: Wallet) {
    if (! await syncWallet.isSigningKeySet()) {
        const startTime = new Date().getTime();
        const changePubkeyHandle = await syncWallet.setSigningKey();
        console.log(`Change pubkey offchain posted: ${(new Date().getTime()) - startTime} ms`);
        await changePubkeyHandle.awaitReceipt();
        console.log(`Change pubkey offchain committed: ${(new Date().getTime()) - startTime} ms`);
        if (! await syncWallet.isSigningKeySet()) {
            throw new Error("Change pubkey offchain failed");
        }
    }
}

async function testThrowingErrorOnTxFail(zksyncDepositorWallet: Wallet) {
    let testPassed = true;

    const ethWallet = ethers.Wallet.createRandom().connect(ethersProvider);
    const syncWallet = await Wallet.fromEthSigner(
        ethWallet,
        syncProvider,
    );
    
    try {
        const tx = await syncWallet.syncTransfer({
            to: zksyncDepositorWallet.address(),
            token: "ETH",
            amount: utils.parseEther('0.01'),
            fee: utils.parseEther('10'),
        });
        await tx.awaitVerifyReceipt();
        testPassed = false;
    } catch (e) {
        console.log('Error (expected) on sync tx fail:', e);
    }

    if (!testPassed) {
        throw new Error("testThrowingErrorOnTxFail failed");
    }
}

async function moveFunds(contract: Contract, ethProxy: ETHProxy, depositWallet: Wallet, syncWallet1: Wallet, syncWallet2: Wallet, token: types.TokenLike, depositAmountETH: string) {
    const depositAmount = utils.parseEther(depositAmountETH);

    // we do two transfers to test transfer to new and ordinary transfer.
    const transfersAmount = depositAmount.div(6);
    const transfersFee = await syncProvider.getTransactionFee("Transfer", transfersAmount, token);


    const withdrawAmount = transfersAmount.div(6);
    const withdrawFee = await syncProvider.getTransactionFee("Withdraw", withdrawAmount, token);

    await testAutoApprovedDeposit(depositWallet, syncWallet1, token, depositAmount.div(2));
    console.log(`Auto approved deposit ok, Token: ${token}`);
    await testDeposit(depositWallet, syncWallet1, token, depositAmount.div(2));
    console.log(`Forever approved deposit ok, Token: ${token}`);
    await testChangePubkeyOnchain(syncWallet1);
    console.log(`Change pubkey onchain ok`);
    await testTransfer(syncWallet1, syncWallet2, token, transfersAmount, transfersFee);
    console.log(`Transfer to new ok, Token: ${token}`);
    await testTransfer(syncWallet1, syncWallet2, token, transfersAmount, transfersFee);
    console.log(`Transfer ok, Token: ${token}`);
    await testTransferToSelf(syncWallet1, token, transfersAmount, transfersFee);
    console.log(`Transfer to self with fee ok, Token: ${token}`);
    await testTransferToSelf(syncWallet1, token, transfersAmount, bigNumberify(0));
    console.log(`Transfer to self no fee ok, Token: ${token}`);
    await testChangePubkeyOffchain(syncWallet2);
    console.log(`Change pubkey offchain ok`);
    await testWithdraw(contract, syncWallet2, syncWallet2, token, withdrawAmount, withdrawFee);
    console.log(`Withdraw ok, Token: ${token}`);
}

(async () => {
    try {
        syncProvider = await Provider.newWebsocketProvider(process.env.WS_API_ADDR);
        const ERC20_ADDRESS = process.env.TEST_ERC20;
        const ERC20_SYMBOL = syncProvider.tokenSet.resolveTokenSymbol(ERC20_ADDRESS);

        const ethProxy = new ETHProxy(ethersProvider, syncProvider.contractAddress);

        const ethWallet = ethers.Wallet.fromMnemonic(
            MNEMONIC,
            "m/44'/60'/0'/0/0"
        ).connect(ethersProvider);
        const syncDepositorWallet = ethers.Wallet.createRandom().connect(ethersProvider);
        await (await ethWallet.sendTransaction({to: syncDepositorWallet.address, value: parseEther("0.5")})).wait();
        const erc20contract = new Contract(ERC20_ADDRESS, IERC20_INTERFACE, ethWallet);
        await (await erc20contract.transfer(syncDepositorWallet.address, parseEther("0.1"))).wait();
        const zksyncDepositorWallet = await Wallet.fromEthSigner(syncDepositorWallet, syncProvider);

        const syncWalletSigner = ethers.Wallet.createRandom().connect(ethersProvider);
        await (await ethWallet.sendTransaction({to: syncWalletSigner.address, value: parseEther("0.05")}));
        const syncWallet = await Wallet.fromEthSigner(
            syncWalletSigner,
            syncProvider,
        );

        const contract = new Contract(
            syncProvider.contractAddress.mainContract,
            franklin_abi.interface,
            ethWallet,
        );

        const ethWallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
        await (await ethWallet.sendTransaction({to: ethWallet2.address, value: parseEther("0.05")}));
        const syncWallet2 = await Wallet.fromEthSigner(
            ethWallet2,
            syncProvider,
        );

        const ethWallet3 = ethers.Wallet.createRandom().connect(ethersProvider);
        await (await ethWallet.sendTransaction({to: ethWallet3.address, value: parseEther("0.01")}));
        const syncWallet3 = await Wallet.fromEthSigner(
            ethWallet3,
            syncProvider,
        );

        await testThrowingErrorOnTxFail(zksyncDepositorWallet);

        await moveFunds(contract, ethProxy, zksyncDepositorWallet, syncWallet, syncWallet2, ERC20_ADDRESS, "0.018");
        await moveFunds(contract, ethProxy, zksyncDepositorWallet, syncWallet, syncWallet2, ERC20_SYMBOL, "0.018");
        await moveFunds(contract, ethProxy, zksyncDepositorWallet, syncWallet, syncWallet3, "ETH", "0.018");

        await syncProvider.disconnect();
    } catch (e) {
        console.error("Error: ", e);
        process.exit(0); // TODO: undestand why it does not work on CI and fix(task is created).
    }
})();
