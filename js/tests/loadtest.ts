import {ethers, utils} from "ethers";
import {
    ETHProxy,
    getDefaultProvider,
    Provider,
    types, utils as syncutils, Wallet,
} from "zksync";

let syncProvider: Provider;

let CLIENTS_TOTAL = 2;
const DEPOSIT_AMOUNT = "0.001";
const TRANSFER_DIVISOR = 1000;
const FEE_DIVISOR = 50;
const TRANSFER_NUM_MULTIPLIER=10;

(async () => {
    const baseWalletPath = "m/44'/60'/0'/0/";

    const WEB3_URL = process.env.WEB3_URL;

    // const ERC20_TOKEN = process.env.TEST_ERC20;
    const tokens = ["ETH"];

    const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";
    console.log(`Running loadtest on the ${network} network`);

    syncProvider = await getDefaultProvider(network);
    // syncProvider = await Provider.newWebsocketProvider("wss://stage-api.zksync.dev/jsrpc-ws");

    const depositAmount = utils.parseEther(DEPOSIT_AMOUNT);
    const transferAmount = depositAmount.div(CLIENTS_TOTAL * TRANSFER_DIVISOR);

    try {
        const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);

        const ethWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, baseWalletPath + "0").connect(ethersProvider);
        const syncWallet = await Wallet.fromEthSigner(ethWallet, syncProvider);

        const syncWallets = [];

        syncWallets.push(syncWallet);

        // Create wallets
        for (let i = 1; i < CLIENTS_TOTAL; i++) {
            const ew = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, `${baseWalletPath}${i}`).connect(ethersProvider);
            const sw = await Wallet.fromEthSigner(ew, syncProvider);
            syncWallets.push(sw);
        }

        // Deposits
        await deposit(syncWallets[0], tokens, depositAmount);

        // Transfers to new
        for (let i = 1; i < CLIENTS_TOTAL * 2; i *= 2) {
            let promises = [];
            const senders_total = ~~(i / 2);
            for (let j = 0; j < senders_total; j++) {
                if (senders_total + j >= CLIENTS_TOTAL) {
                    break;
                }
                promises.push(transfer1to1(syncWallets[j], syncWallets[senders_total + j], tokens, depositAmount.div(i + 1)));
                console.log(`Initiated transfer to new: ${j} to ${senders_total + j}`);
            }
            let results = await Promise.all(promises.map(reflect));

            let failedPromises = results.filter((p) => p.status === "rejected");
            for (const promise of failedPromises) {
                console.log(`Failed transfer: ${promise.reason}`);
            }
        }

        // Transfers
        let promises = [];

        for (const wallet of syncWallets) {
            promises.push(transfer1toAll(wallet, syncWallets, tokens, transferAmount));
        }
        let results = await Promise.all(promises.map(reflect));

        let failedPromises = results.filter((p) => p.status === "rejected");
        for (const promise of failedPromises) {
            console.log(`Failed transfer: ${promise.reason}`);
        }

        // Withdraws
        promises = [];
        results = [];
        failedPromises = [];

        for (const wallet of syncWallets) {
            promises.push(withdraw(wallet, tokens));
        }
        await Promise.all(promises.map(reflect));

        results = await Promise.all(promises.map(reflect));
        failedPromises = results.filter((p) => p.status === "rejected");
        for (const promise of failedPromises) {
            console.log(`Failed withdraw: ${promise.reason}`);
        }

        console.log(`Finished loadtest`);
    } catch (err) {
        console.log(`Failed: ${err}`);
    } finally {
        await syncProvider.disconnect();
    }
})();


async function deposit(syncWallet: Wallet, tokens: types.TokenLike[], amount: utils.BigNumber) {
    try {
        for (const token of tokens) {
            const depositHandle = await syncWallet.depositToSyncFromEthereum({
                depositTo: syncWallet.address(),
                token,
                amount,
            });
            await depositHandle.awaitReceipt();

            console.log(`${token} deposit ok, from: ${syncWallet.address}, to: ${syncWallet.address()}, amount: ${utils.formatEther(amount)}`);
        }

    } catch (err) {
        console.log(`Deposit error: ${err}`);
        throw err;
    }
}

async function transfer1to1(fromWallet: Wallet, toWallet: Wallet, tokens: types.TokenLike[], amount: utils.BigNumber) {
    try {
        const transferAmount = syncutils.closestPackableTransactionAmount(amount);
        const fee = syncutils.closestPackableTransactionFee(transferAmount.div(FEE_DIVISOR));

        if (toWallet.address() !== fromWallet.address()) {
            for (const token of tokens) {
                if (!await fromWallet.isSigningKeySet()) {
                    await (await fromWallet.setSigningKey()).awaitReceipt();
                }
                const tx = await fromWallet.syncTransfer({
                    to: toWallet.address(),
                    token,
                    amount: transferAmount,
                    fee,
                });
                await tx.awaitReceipt();
                console.log(`${token} transfer ok, from: ${fromWallet.address()}, to: ${toWallet.address()}, amount: ${utils.formatEther(amount)}, fee: ${utils.formatEther(fee)}`);
            }
        }

    } catch (err) {
        console.log(`Transfer error: ${err}`);
        throw err;
    }
}

async function transfer1toAll(fromWallet: Wallet, toWallets: Wallet[], tokens: types.TokenLike[], amount: utils.BigNumber) {
    try {
        const transferAmount = syncutils.closestPackableTransactionAmount(amount);
        const fee = syncutils.closestPackableTransactionFee(transferAmount.div(FEE_DIVISOR));

        for (let t = 0; t < TRANSFER_NUM_MULTIPLIER; t++) {
            for (const wallet of toWallets) {
                if (wallet.address() !== fromWallet.address()) {
                    for (const token of tokens) {
                        if (!await fromWallet.isSigningKeySet()) {
                            await (await fromWallet.setSigningKey()).awaitReceipt();
                        }
                        const tx = await fromWallet.syncTransfer({
                            to: wallet.address(),
                            token,
                            amount: transferAmount,
                            fee,
                        });
                        await tx.awaitReceipt();
                        console.log(`${token} transfer ok, from: ${fromWallet.address()}, to: ${wallet.address()}, amount: ${utils.formatEther(amount)}, fee: ${utils.formatEther(fee)}`);
                    }
                }
            }
        }
    } catch (err) {
        console.log(`Transfer error: ${err}`);
        throw err;
    }
}

async function withdraw(syncWallet: Wallet, tokens: types.TokenLike[]) {
    try {
        for (const token of tokens) {
            const wallet2BeforeWithdraw = await syncWallet.getBalance(token);
            const fee = syncutils.closestPackableTransactionFee(wallet2BeforeWithdraw.div(FEE_DIVISOR));
            const amount = wallet2BeforeWithdraw.sub(fee);
            const withdrawHandle = await syncWallet.withdrawFromSyncToEthereum({
                ethAddress: syncWallet.address(),
                token,
                amount,
                fee,
            });
            await withdrawHandle.awaitReceipt();

            console.log(`${token} withdraw ok, from: ${syncWallet.address()}, to: ${syncWallet.address}, amount: ${utils.formatEther(amount)}, fee: ${utils.formatEther(fee)}`);
        }
    } catch (err) {
        console.log(`Withdraw error: ${err}`);
        throw err;
    }
}

function reflect(promise) {
    return promise.then(
        (result) => {
            return {status: "fulfilled", value: result};
        },
        (error) => {
            return {status: "rejected", reason: error};
        },
    );
}
