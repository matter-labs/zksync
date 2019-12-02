import {
    depositFromETH,
    Wallet,
    Provider,
    ETHProxy, getDefaultProvider, types
} from "zksync";
import { ethers, utils } from "ethers";


let syncProvider: Provider;

async function getOperatorBalance(token: types.Token, type: "committed" | "verified" = "committed") {
    const accountState = await syncProvider.getState(process.env.OPERATOR_FRANKLIN_ADDRESS);
    if (token != "ETH") {
        token = token.toLowerCase();
    }
    let balance;
    if (type == "committed") {
        balance = accountState.committed.balances[token] || "0";
    } else {
        balance = accountState.verified.balances[token] || "0";
    }
    return utils.bigNumberify(balance);
}

async function testDeposit(ethWallet: ethers.Signer, syncWallet: Wallet, token: types.Token, amount: utils.BigNumberish) {
    const balanceBeforeDep = await syncWallet.getBalance(token);
    const depositHandle = await depositFromETH(
    {
        depositFrom: ethWallet,
        depositTo:  syncWallet,
        token: token,
        amount,
        maxFeeInETHToken: utils.parseEther("0.1")
    });
    await depositHandle.awaitReceipt();
    const balanceAfterDep = await syncWallet.getBalance(token);

    if (!balanceAfterDep.sub(balanceBeforeDep).eq(amount)) {
        throw new Error("Deposit checks failed");
    }
}

async function testTransfer(syncWallet1: Wallet, syncWallet2: Wallet, token: types.Token, amount: utils.BigNumber, fee: utils.BigNumber) {
    const wallet1BeforeTransfer = await syncWallet1.getBalance(token);
    const wallet2BeforeTransfer = await syncWallet2.getBalance(token);
    const operatorBeforeTransfer = await getOperatorBalance(token);
    const transferToNewHandle = await syncWallet1.syncTransfer({
        to: syncWallet2.address(),
        token,
        amount,
        fee
    });
    await transferToNewHandle.awaitReceipt();
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

async function testWithdraw(ethWallet: ethers.Wallet, syncWallet: Wallet, token: types.Token, amount: utils.BigNumber, fee: utils.BigNumber) {
    const wallet2BeforeWithdraw = await syncWallet.getBalance(token);
    const operatorBeforeWithdraw = await getOperatorBalance(token);
    const withdrawHandle = await syncWallet.withdrawTo({
        ethAddress: ethWallet.address,
        token,
        amount,
        fee
    });
    await withdrawHandle.awaitReceipt();
    const wallet2AfterWithdraw = await syncWallet.getBalance(token);
    const operatorAfterWithdraw = await getOperatorBalance(token);

    let withdrawCorrect = true;
    withdrawCorrect = withdrawCorrect && wallet2BeforeWithdraw.sub(wallet2AfterWithdraw).eq(amount.add(fee));
    withdrawCorrect = withdrawCorrect && operatorAfterWithdraw.sub(operatorBeforeWithdraw).eq(fee);

    if (!withdrawCorrect) {
        throw new Error("Withdraw checks failed");
    }
}

async function moveFunds(wallet1: ethers.Wallet, syncWallet1: Wallet, wallet2: ethers.Wallet, syncWallet2: Wallet, token: types.Token, depositAmountETH: string) {
    const depositAmount = utils.parseEther(depositAmountETH);

    // we do two transfers to test transfer to new and ordinary transfer.
    const transfersFee = depositAmount.div(25);
    const transfersAmount = depositAmount.div(2).sub(transfersFee);

    const withdrawFee = transfersAmount.div(20);
    const withdrawAmount = transfersAmount.sub(withdrawFee);

    await testDeposit(wallet1, syncWallet1, token, depositAmount);
    console.log(`Deposit ok, Token: ${token}`);
    await testTransfer(syncWallet1, syncWallet2, token, transfersAmount, transfersFee);
    console.log(`Transfer to new ok, Token: ${token}`);
    await testTransfer(syncWallet1, syncWallet2, token, transfersAmount, transfersFee);
    console.log(`Transfer ok, Token: ${token}`);
    await testWithdraw(wallet2, syncWallet2, token, withdrawAmount, withdrawFee);
    console.log(`Withdraw ok, Token: ${token}`);
}

(async () => {
    const WEB3_URL = process.env.WEB3_URL;
// Mnemonic for eth wallet.
    const MNEMONIC = process.env.MNEMONIC;
    const ERC_20TOKEN = process.env.TEST_ERC20;

    syncProvider = await getDefaultProvider("localhost");

    const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
    const ethProxy = new ETHProxy(ethersProvider, syncProvider.contractAddress);

    const ethWallet = ethers.Wallet.fromMnemonic(
        MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    const syncWallet = await Wallet.fromEthWallet(
        ethWallet,
        syncProvider,
        ethProxy
    );

    const ethWallet2 = ethers.Wallet.createRandom().connect(ethersProvider);
    const syncWallet2 = await Wallet.fromEthWallet(
        ethWallet2,
        syncProvider,
        ethProxy
    );

    const ethWallet3 = ethers.Wallet.createRandom().connect(ethersProvider);
    const syncWallet3 = await Wallet.fromEthWallet(
        ethWallet3,
        syncProvider,
        ethProxy
    );

    await moveFunds(ethWallet, syncWallet, ethWallet2, syncWallet2, ERC_20TOKEN, "0.01");
    await moveFunds(ethWallet, syncWallet, ethWallet3, syncWallet3, "ETH", "0.01");

    await syncProvider.disconnect();
})();
