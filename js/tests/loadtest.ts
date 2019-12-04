import {
    utils as syncutils,
    depositFromETH,
    Wallet,
    Provider,
    ETHProxy, getDefaultProvider, types
} from "zksync";
import { ethers, utils } from "ethers";

let syncProvider: Provider;

const CLIENTS_TOTAL = 2;
const DEPOSIT_AMOUNT = "0.000000000000001";
const TRANSFER_DIVISOR = 1000;
const FEE_DIVISOR = 50;

(async () => {
    const WEB3_URL = process.env.WEB3_URL;
    const TEST_ACCOUNT_PRIVATE_KEY = process.env.TEST_ACCOUNT_PRIVATE_KEY;
    const ERC20_TOKEN = process.env.TEST_ERC20;

    const network = process.env.ETH_NETWORK == "localhost" ? "localhost" : "testnet";
    console.log("Running loadtest on the ", network, " network");

    syncProvider = await getDefaultProvider(network);

    const depositAmount = utils.parseEther(DEPOSIT_AMOUNT);

    try {
        const ethersProvider = new ethers.providers.JsonRpcProvider(WEB3_URL);
        const ethProxy = new ETHProxy(ethersProvider, syncProvider.contractAddress);

        const ethWallet = new ethers.Wallet(TEST_ACCOUNT_PRIVATE_KEY, ethersProvider);

        const syncWallet = await Wallet.fromEthSigner(
            ethWallet,
            syncProvider,
            ethProxy
        );

        const ethWallets = [];
        const syncWallets = [];

        ethWallets.push(ethWallet);
        syncWallets.push(syncWallet);

        // Create wallets
        for(let i = 1; i < CLIENTS_TOTAL; i++) {
            let ew = ethers.Wallet.createRandom().connect(ethersProvider);
            let sw = await Wallet.fromEthSigner(
                ew,
                syncProvider,
                ethProxy
            );
            ethWallets.push(ew);
            syncWallets.push(sw);
        }

        // Deposits
        await deposit(ethWallets[0], syncWallets, ["ETH", ERC20_TOKEN], depositAmount);

        // Transfers
        let promises = [];
        for(let i = 0; i < CLIENTS_TOTAL; i++) {
            promises.push(transfer(syncWallets[i], syncWallets, ["ETH", ERC20_TOKEN], depositAmount.div(TRANSFER_DIVISOR)));
        }
        await Promise.all(promises);

        // Withdraws
        promises = [];
        for(let i = 0; i < CLIENTS_TOTAL; i++) {
            promises.push(withdraw(ethWallets[i], syncWallets[i], ["ETH", ERC20_TOKEN]));
        }
        await Promise.all(promises);

        await syncProvider.disconnect();
    } catch (err) {
        await syncProvider.disconnect();
        throw err
    } 

})();

async function deposit(ethWallet: ethers.Wallet, syncWallets: Wallet[], tokens: types.Token[], amount: utils.BigNumber) {
    try {
        for (let i = 0; i < syncWallets.length; i++) {
            for (let k = 0; k < tokens.length; k++) {
                const depositHandle = await depositFromETH(
                {
                    depositFrom: ethWallet,
                    depositTo:  syncWallets[i],
                    token: tokens[k],
                    amount: amount
                });
                await depositHandle.awaitReceipt();

                console.log(`${tokens[k]} deposit ok, from: ${ethWallet.address}, to: ${syncWallets[i].address()}, amount: ${utils.formatEther(amount)}`);
            }
        }
        
    } catch (err) {
        console.log(`Deposit error: ${err}`)
        throw err
    }
}

async function transfer(fromWallet: Wallet, toWallets: Wallet[], tokens: types.Token[], amount: utils.BigNumber) {
    try {
        const transferAmount = syncutils.closestPackableTransactionAmount(amount);
        const fee = syncutils.closestPackableTransactionFee(transferAmount.div(FEE_DIVISOR));
       
        for (let i = 0; i < toWallets.length; i++) {
            if (toWallets[i].address() != fromWallet.address()) {
                for (let k = 0; k < tokens.length; k++) {
                    const tx = await fromWallet.syncTransfer({
                        to: toWallets[i].address(),
                        token: tokens[k],
                        amount: transferAmount,
                        fee
                    });
            
                    await tx.awaitReceipt();
                    console.log(`${tokens[k]} transfer ok, from: ${fromWallet.address()}, to: ${toWallets[i].address()}, amount: ${utils.formatEther(amount)}, fee: ${utils.formatEther(fee)}`);
                }
            }
        }

    } catch (err) {
        console.log(`Transfer error: ${err}`)
        throw err
    }
}

async function withdraw(ethWallet: ethers.Wallet, syncWallet: Wallet, tokens: types.Token[]) {
    try {
        for (let k = 0; k < tokens.length; k++) {
            const wallet2BeforeWithdraw = await syncWallet.getBalance(tokens[k]);
            const fee = syncutils.closestPackableTransactionFee(wallet2BeforeWithdraw.div(FEE_DIVISOR));
            const amount = wallet2BeforeWithdraw.sub(fee);
            const withdrawHandle = await syncWallet.withdrawTo({
                ethAddress: ethWallet.address,
                token: tokens[k],
                amount,
                fee
            });
            await withdrawHandle.awaitReceipt();
    
            console.log(`${tokens[k]} withdraw ok, from: ${syncWallet.address()}, to: ${ethWallet.address}, amount: ${utils.formatEther(amount)}, fee: ${utils.formatEther(fee)}`);
        }
    } catch (err) {
        console.log(`Withdraw error: ${err}`)
        throw err
    }
}
