import { ethers } from "ethers";
import {
    addTestERC20Token,
    mintTestERC20Token,
    franklinTestContractCode,
    verifierTestContractCode,
    governanceTestContractCode,
    priorityQueueTestContractCode,
    Deployer,
} from "../src.ts/deploy";

import { expect, use } from "chai";
import { createMockProvider, getWallets, solidity, deployContract } from "ethereum-waffle";
import { bigNumberify, parseEther, hexlify, formatEther } from "ethers/utils";
import {
    withdrawEthFromContract,
    postFullExit,
    postEthDeposit,
    postErc20Deposit,
    postBlockCommit,
    postBlockVerify,
    createDepositPublicData,
    createWithdrawPublicData,
    createFullExitPublicData,
} from "./helpers";

use(solidity);

// geth version

// const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
// const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
// const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);

// ganache version

const provider = createMockProvider();
const [wallet, exitWallet]  = getWallets(provider);

const franklinAddress = "0809101112131415161718192021222334252627";
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("Integration test", async function () {
    this.timeout(50000);

    const deployer = new Deployer(wallet, true);
    let franklinDeployedContract;
    let governanceDeployedContract;
    let erc20DeployedToken;
    let verifierDeployedContract;
    let priorityQueueDeployedContract;

    before(async () => {
        //console.log("---\n");
        verifierDeployedContract = await deployer.deployVerifier();
        governanceDeployedContract = await deployer.deployGovernance();
        franklinDeployedContract = await deployer.deployFranklin();
        await governanceDeployedContract.setValidator(wallet.address, true);
        erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, erc20DeployedToken);
        // Make sure that exit wallet can execute transactions.
        await wallet.sendTransaction({ to: exitWallet.address, value: parseEther("1.0") });
    });

    const tokenId = 0;
    const tokenAddr = "0x0000000000000000000000000000000000000000";

    const value = parseEther("0.3"); // the value passed to tx
    const depositAmount = parseEther("0.296778"); // amount after: tx value - some counted fee
    const depositFee = parseEther("0.003222"); // tx fee

    it("should make a deposit", async () => {
        //console.log("")
        //console.log(" - ETH Integration started");

        // Deposit eth
        await postEthDeposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            depositAmount,
            depositFee,
            franklinAddress,
            value,
            null,
        );

        //console.log("Requested deposit");
    })

    const depositBlockPublicData = createDepositPublicData(0, hexlify(depositAmount), franklinAddress);
    const feeAccount = 22;
    const root = "0000000000000000000000000000000000000000000000000000000000000000";

    it("should make a commitment to deposit", async () => {

        // Commit block with eth deposit
        let commitment = "0xc456a531f6b89e6c0bf3a381b03961725895447203ec77cb0a2afd95e78217dd";
        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            1,
            feeAccount,
            root,
            depositBlockPublicData,
            1,
            1,
            commitment,
            null,
        );

        //console.log("Deposit committed");
    })


    // Commit block with eth partial exit.
    const exitValue = parseEther("0.2");
    const exitBlockPublicData = createWithdrawPublicData(tokenId, hexlify(exitValue), exitWallet.address);

    it("should make partial exit", async () => {

        let commitment = "0xebea7f6ebc71aeb2febfbd750ec46f513d1e527c2bf5a98d7f65e3bbbb285dcb";

        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            2,
            feeAccount,
            root,
            exitBlockPublicData,
            1,
            0,
            commitment,
            null,
        );

        //console.log("Partial exit committed");
    
    })

    it("should verify", async () => {

        // const beforePartExitBalance = await exitWallet.getBalance();
        // const afterPartExitBalance = await exitWallet.getBalance();

        // Verify block with deposit and partial exit.
        await postBlockVerify(
            provider,
            franklinDeployedContract,
            1,
            dummyBlockProof,
            null,
        );

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(1);

        //console.log("Verified deposit");

        await postBlockVerify(
            provider,
            franklinDeployedContract,
            2,
            dummyBlockProof,
            null,
        );

    })

    it("should complete withdrawal", async () => {

        await (await franklinDeployedContract.completeWithdrawals(1)).wait();

        //expect(afterPartExitBalance.sub(beforePartExitBalance)).eq(exitValue);

        //console.log("Verified partial exit");

    })

    it("ETH full exit, commit, verify", async () => {

        // Full exit eth
        const fullExitAmount = parseEther("0.096778"); // amount after: tx value - some counted fee - exit amount
        const accId = 0;
        const pubkey = "0x0000000000000000000000000000000000000000000000000000000000000000";
        const signature = Buffer.from("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "hex");
        const nonce = 0;
        await postFullExit(
            provider,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            accId,
            pubkey,
            tokenAddr,
            signature,
        );

        //console.log("Full exit requested");

        // Commit block with full exit
        const fullExitBlockPublicData = createFullExitPublicData(
            accId, wallet.address, tokenId, hexlify(fullExitAmount),
        );
        let commitment = "0xf8d56172b22427e926843b478edfb630bfdd45b6d7828cf1720ba0ace089947c";
        await postBlockCommit(
            provider,
            wallet,
            franklinDeployedContract,
            3,
            feeAccount,
            root,
            fullExitBlockPublicData,
            1,
            1,
            commitment,
            null,
        );

        //console.log("Full exit committed");

        // Verify block with full exit.
        const beforeFullExitBalance = await wallet.getBalance();

        await postBlockVerify(
            provider,
            franklinDeployedContract,
            3,
            dummyBlockProof,
            null,
        );

    })

    it("should withdraw", async () => {

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(2);

        // Withdraw accumulated fees eth for wallet
        const balanceToWithdraw = await franklinDeployedContract.getBalanceToWithdraw(wallet.address, 0);

        await withdrawEthFromContract(
            provider,
            wallet,
            franklinDeployedContract,
            balanceToWithdraw,
            null,
        );

        expect(await franklinDeployedContract.getBalanceToWithdraw(wallet.address, 0)).equal(bigNumberify(0));

        //console.log("Full exit verified and withdrawed to wallet");
        //console.log(" + ETH Integration passed")
    });

    const amount = 78; // the value passed to tx
    const value2 = parseEther("0.3"); // we send in tx value
    const depositFee2 = parseEther("0.003852"); // tx fee get from fee value

    it("should make ERC20 token deposit", async () => {

        //console.log("\n - ERC20 Integration started");

        const tokenId = 1;
        const tokenAddr = erc20DeployedToken.address;

        // Deposit eth
        await postErc20Deposit(
            provider,
            wallet,
            franklinDeployedContract,
            priorityQueueDeployedContract,
            erc20DeployedToken,
            amount,
            depositFee,
            franklinAddress,
            value,
            null,
        );

        //console.log("Requested deposit");
    })

    // unused for now

    // it("should commit deposit", async () => {

    //     // Commit block with eth deposit
    //     const depositBlockPublicData = createDepositPublicData(1, hexlify(amount), franklinAddress);
    //     const feeAccount = 22;
    //     const root = "0000000000000000000000000000000000000000000000000000000000000000";
    //     let commitment = "0x7d7043f2983872e7d5632d181b0a8e0308c921b4e12ac24d69eb49def9a67c33";
    //     await postBlockCommit(
    //         provider,
    //         wallet,
    //         franklinDeployedContract,
    //         1,
    //         feeAccount,
    //         root,
    //         depositBlockPublicData,
    //         1,
    //         1,
    //         commitment,
    //         null,
    //     );

    // })

    //it("ERC20 deposit, part exit, full exit, commit, verify, withdraw", async () => {

    //     // Commit block with eth partial exit.
    //     const exitValue = 2;
    //     const exitBlockPublicData = createWithdrawPublicData(1, hexlify(exitValue), exitWallet.address);
    //     commitment = "0xec9702b125356faae38041a7fde0094af09f2f60997f3148a86217999f1221ea";

    //     await postBlockCommit(
    //         provider,
    //         wallet,
    //         franklinDeployedContract,
    //         2,
    //         feeAccount,
    //         root,
    //         exitBlockPublicData,
    //         1,
    //         0,
    //         commitment,
    //         null,
    //     );

    //     console.log("Partial exit committed");

    //     // Verify block with deposit and partial exit.
    //     await postBlockVerify(
    //         provider,
    //         franklinDeployedContract,
    //         1,
    //         dummyBlockProof,
    //         null,
    //     );

    //     expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
    //     expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(1);

    //     console.log("Verified deposit");

    //     const oldBalance1 = await erc20DeployedToken.balanceOf(exitWallet.address);

    //     await postBlockVerify(
    //         provider,
    //         franklinDeployedContract,
    //         2,
    //         dummyBlockProof,
    //         null,
    //     );

    //     await (await franklinDeployedContract.completeWithdrawals(1)).wait();
    //     const newBalance1 = await erc20DeployedToken.balanceOf(exitWallet.address);

    //     expect(newBalance1.sub(oldBalance1)).eq(exitValue);

    //     console.log("Verified partial exit");

    //     // Full exit erc
    //     const fullExitAmount = 76; // amount after: tx value - some counted fee - exit amount
    //     const accId = 0;
    //     const pubkey = "0x0000000000000000000000000000000000000000000000000000000000000000";
    //     const signature = Buffer.from("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "hex");
    //     const nonce = 0;
    //     await postFullExit(
    //         provider,
    //         franklinDeployedContract,
    //         priorityQueueDeployedContract,
    //         accId,
    //         pubkey,
    //         tokenAddr,
    //         signature,
    //         nonce,
    //         value,
    //         null,
    //     );

    //     console.log("Full exit requested");

    //     // Commit block with full exit
    //     const fullExitBlockPublicData = createFullExitPublicData(
    //         accId, wallet.address, tokenId, hexlify(fullExitAmount),
    //     );
    //     commitment = "0x10a7e3614ba95ff093b826f78886f190a26bd16129faaec145ffbf78d3cfdf5e";
    //     await postBlockCommit(
    //         provider,
    //         wallet,
    //         franklinDeployedContract,
    //         3,
    //         feeAccount,
    //         root,
    //         fullExitBlockPublicData,
    //         1,
    //         1,
    //         commitment,
    //         null,
    //     );

    //     console.log("Full exit committed");

    //     // Verify block with full exit.
    //     const oldBalance2 = await erc20DeployedToken.balanceOf(wallet.address);

    //     await postBlockVerify(
    //         provider,
    //         franklinDeployedContract,
    //         3,
    //         dummyBlockProof,
    //         null,
    //     );

    //     expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
    //     expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(2);

    //     await (await franklinDeployedContract.completeWithdrawals(1)).wait();
    //     const newBalance2 = await erc20DeployedToken.balanceOf(wallet.address);

    //     expect(newBalance2.sub(oldBalance2)).eq(fullExitAmount);

    //     console.log("Full exit verified");

    //     console.log(" + ERC20 Integration passed");
    //});
});
