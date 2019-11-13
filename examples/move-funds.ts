import BN = require("bn.js");
import { depositFromETH, Wallet } from "../src/wallet";
import { Contract, ethers, utils } from "ethers";
import { bigNumberify, formatEther, parseEther } from "ethers/utils";
import { ETHProxy, SyncProvider } from "../src/provider";
import { WSTransport } from "../src/transport";
import { Token } from "../src/types";

function shortAddr(address: string): string {
    return `${address.substr(0, 6)}`;
}

const IERC20ConractInterface = new utils.Interface(
    require("../abi/IERC20.json").interface
);

async function logSyncBalance(
    wallet: Wallet,
    token: Token,
    type: "commited" | "verified" = "commited"
) {
    const balance = formatEther(await wallet.getBalance(token, type));
    console.log(
        `SYNC:${shortAddr(
            wallet.address()
        )} ${type} balance: ${balance} ${token}`
    );
}
async function logETHBalance(wallet: ethers.Wallet, token: Token) {
    let balance;
    if (token == "ETH") {
        balance = formatEther(await wallet.getBalance());
    } else {
        const erc20contract = new Contract(
            token,
            IERC20ConractInterface,
            wallet
        );
        balance = formatEther(await erc20contract.balanceOf(wallet.address));
    }

    console.log(
        `ETH:${shortAddr(wallet.address)} balance: ${balance} ${token}`
    );
}

(async () => {
    const ethersProvider = new ethers.providers.JsonRpcProvider(
        process.env.WEB3_URL
    );
    const wsSidechainProvider = await SyncProvider.newWebsocketProvider();
    const ethProxy = new ETHProxy(
        ethersProvider,
        wsSidechainProvider.contractAddress
    );

    console.log("Contract address: ", wsSidechainProvider.contractAddress);

    const ethWallet = ethers.Wallet.fromMnemonic(
        process.env.MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    const wallet = await Wallet.fromEthWallet(
        ethWallet,
        wsSidechainProvider,
        ethProxy
    );

    const ethWallet2 = ethers.Wallet.fromMnemonic(
        process.env.MNEMONIC,
        "m/44'/60'/0'/0/2"
    ).connect(ethersProvider);
    const wallet2 = await Wallet.fromEthWallet(
        ethWallet2,
        wsSidechainProvider,
        ethProxy
    );

    const depositAmount = "0.1";
    const depositToken = process.env.TEST_ERC20;
    console.log("==================================");
    console.log(
        `Deposit: ${depositAmount} ${depositToken}: ETH:${shortAddr(
            ethWallet.address
        )} -> SYNC:${shortAddr(wallet.address())}`
    );
    await logETHBalance(ethWallet, depositToken);
    await logSyncBalance(wallet, depositToken, "verified");

    const depositHandle = await depositFromETH(
        ethWallet,
        wallet,
        depositToken,
        utils.parseEther(depositAmount),
        utils.parseEther("0.1")
    );
    await depositHandle.waitCommit();
    console.log("Deposit commited");

    await logETHBalance(ethWallet, depositToken);
    await logSyncBalance(wallet, depositToken, "commited");

    console.log("==================================");
    console.log(
        `Transfer: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            wallet.address()
        )} -> SYNC:${shortAddr(wallet2.address())}`
    );
    await logSyncBalance(wallet, depositToken, "commited");
    await logSyncBalance(wallet2, depositToken, "commited");

    const transferHandle = await wallet.syncTransfer(
        wallet2.address(),
        depositToken,
        utils.parseEther(depositAmount),
        0
    );
    await transferHandle.waitCommit();
    console.log("Transfer commited");

    await logSyncBalance(wallet, depositToken, "commited");
    await logSyncBalance(wallet2, depositToken, "commited");

    console.log("==================================");
    console.log(
        `Withdraw: ${depositAmount} ${depositToken}: SYNC:${shortAddr(
            wallet.address()
        )} -> ETH:${shortAddr(ethWallet2.address)}`
    );
    await logSyncBalance(wallet2, depositToken);
    await logETHBalance(ethWallet2, depositToken);

    const withdrawHandle = await wallet2.withdrawTo(
        ethWallet2.address,
        depositToken,
        ethers.utils.parseEther(depositAmount),
        0
    );
    await withdrawHandle.waitVerify();
    console.log("Withdraw commited");

    await logSyncBalance(wallet2, depositToken, "verified");
    await logETHBalance(ethWallet2, depositToken);

    await (wsSidechainProvider.transport as WSTransport).ws.close();
})();
