import { ethers } from "ethers";
import {
    addTestERC20Token,
    mintTestERC20Token,
    addTestNotApprovedERC20Token,
    deployFranklin,
    deployGovernance,
    deployPriorityQueue,
    deployVerifier,
    franklinTestContractCode,
    verifierTestContractCode,
    governanceTestContractCode,
    priorityQueueTestContractCode,
} from "../src.ts/deploy";

import { expect, use } from "chai";
import { solidity } from "ethereum-waffle";
import { bigNumberify, parseEther, hexlify, formatEther } from "ethers/utils";
import { createDepositPublicData, createWithdrawPublicData, createFullExitPublicData, hex_to_ascii } from "./helpers";

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "0809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("INTEGRATION", function () {
    this.timeout(50000);

    let franklinDeployedContract;
    let governanceDeployedContract;
    let erc20DeployedToken;
    let verifierDeployedContract;
    let priorityQueueDeployedContract;

    beforeEach(async () => {
        console.log("---\n");
        verifierDeployedContract = await deployVerifier(wallet, verifierTestContractCode, []);
        governanceDeployedContract = await deployGovernance(wallet, governanceTestContractCode, [wallet.address]);
        priorityQueueDeployedContract = await deployPriorityQueue(wallet, priorityQueueTestContractCode, [governanceDeployedContract.address]);
        franklinDeployedContract = await deployFranklin(
            wallet,
            franklinTestContractCode,
            [
                governanceDeployedContract.address,
                verifierDeployedContract.address,
                priorityQueueDeployedContract.address,
                wallet.address,
                ethers.constants.HashZero,
            ],
        );
        await governanceDeployedContract.setValidator(wallet.address, true);
        erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, erc20DeployedToken);
        // Make sure that exit wallet can execute transactions.
        await wallet.sendTransaction({ to: exitWallet.address, value: parseEther("1.0") });
    });

    it("ETH deposit, part exit, full exit, commit, verify, withdraw", async () => {
        console.log("\n - ETH Integration started");

        const tokenId = 0;
        const tokenAddr = "0x0000000000000000000000000000000000000000";

        // Deposit eth
        const depositValue = parseEther("0.3"); // the value passed to tx
        const depositAmount = parseEther("0.296778"); // amount after: tx value - some counted fee
        const depositFee = parseEther("0.003222"); // tx fee
        const depositTx = await franklinDeployedContract.depositETH(depositAmount, franklinAddressBinary, { value: depositValue });
        const depositReceipt = await depositTx.wait();
        const depositEvent = depositReceipt.events[1].args;

        expect(depositEvent.owner).equal(wallet.address);
        expect(depositEvent.tokenId).equal(0);
        expect(depositEvent.amount).equal(depositAmount);
        expect(depositEvent.fee).equal(depositFee)
        expect(depositEvent.franklinAddress).equal("0x0809101112131415161718192021222334252627");

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(1);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(0);

        console.log("Requested deposit");

        // Commit block with eth deposit
        const depositBlockPublicData = createDepositPublicData(0, hexlify(depositAmount), franklinAddress);
        const commitTx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        const commitReceipt = await commitTx.wait();
        const commitEvents = commitReceipt.events;

        const commitedEvent1 = commitEvents[0].args;

        expect(commitedEvent1.blockNumber).equal(1);

        expect(await franklinDeployedContract.totalOnchainOps()).equal(1);

        expect((await franklinDeployedContract.blocks(1)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).commitment).equal("0xc456a531f6b89e6c0bf3a381b03961725895447203ec77cb0a2afd95e78217dd");
        expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(1)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Deposit committed");

        // Commit block with eth partial exit.
        const exitValue = parseEther("0.2");

        const exitBlockPublicData = createWithdrawPublicData(tokenId, hexlify(exitValue), exitWallet.address);

        const partExTx = await franklinDeployedContract.commitBlock(2, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            exitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        const partExReceipt = await partExTx.wait();
        const partExEvents = partExReceipt.events;

        const commitedEvent2 = partExEvents.pop().args;

        expect(commitedEvent2.blockNumber).equal(2);

        expect(await franklinDeployedContract.totalOnchainOps()).equal(2);

        expect((await franklinDeployedContract.blocks(2)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(2)).priorityOperations).equal(0);
        expect((await franklinDeployedContract.blocks(2)).commitment).equal("0xebea7f6ebc71aeb2febfbd750ec46f513d1e527c2bf5a98d7f65e3bbbb285dcb");
        expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(2)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Partial exit committed");

        // Verify block with deposit and partial exit.
        const verifyDepTx = await franklinDeployedContract.verifyBlock(1, dummyBlockProof, { gasLimit: bigNumberify("500000") });
        const verifyDepReceipt = await verifyDepTx.wait();
        const verifyDepEvents = verifyDepReceipt.events;

        const verifiedEvent1 = verifyDepEvents.pop().args;

        expect(verifiedEvent1.blockNumber).equal(1);

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(1);

        console.log("Verified deposit");

        const beforePartExitBalance = await exitWallet.getBalance();

        const verifyPartExTx = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, { gasLimit: bigNumberify("500000") });
        const verifyPartExReceipt = await verifyPartExTx.wait();
        const verifyPartExEvents = verifyPartExReceipt.events;

        const verifiedEvent2 = verifyPartExEvents.pop().args;

        expect(verifiedEvent2.blockNumber).equal(2);

        let v = await franklinDeployedContract.completeWithdrawals(1);
        await v.wait();
        const afterPartExitBalance = await exitWallet.getBalance();
        expect(afterPartExitBalance.sub(beforePartExitBalance)).eq(exitValue);

        console.log("Verified partial exit");

        // Full exit eth
        const fullExitAmount = parseEther("0.096778"); // amount after: tx value - some counted fee - exit amount
        const fullExitMinusGas = parseEther("0.096047308");
        const accId = 0;
        const pubkey = "0x0000000000000000000000000000000000000000000000000000000000000000";
        const signature = Buffer.from("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "hex");
        const nonce = 0;
        const fullExTx = await franklinDeployedContract.fullExit(
            accId,
            pubkey,
            tokenAddr,
            signature,
            nonce,
            { value: depositValue, gasLimit: bigNumberify("500000") }
        );
        await fullExTx.wait();

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(1);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(1);

        console.log("Full exit requested");

        // Commit block with full exit
        const fullExitBlockPublicData = createFullExitPublicData(
            accId, wallet.address, tokenId, hexlify(fullExitAmount),
        );
        const commitFullExTx = await franklinDeployedContract.commitBlock(3, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            fullExitBlockPublicData,
            {
                gasLimit: bigNumberify("8000000"),
            },
        );

        const commitFullExReceipt = await commitFullExTx.wait();

        const commitFullExEvents = commitFullExReceipt.events;

        const commitedEvent3 = commitFullExEvents[0].args;

        expect(commitedEvent3.blockNumber).equal(3);

        expect(await franklinDeployedContract.totalOnchainOps()).equal(3);

        expect((await franklinDeployedContract.blocks(3)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).commitment).equal("0xf8d56172b22427e926843b478edfb630bfdd45b6d7828cf1720ba0ace089947c");
        expect((await franklinDeployedContract.blocks(3)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(3)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Full exit committed");

        // Verify block with full exit.
        const beforeFullExitBalance = await wallet.getBalance();

        const verifyFullExTx = await franklinDeployedContract.verifyBlock(3, dummyBlockProof, { gasLimit: bigNumberify("500000") });
        const verifyFullExReceipt = await verifyFullExTx.wait();
        const verifyEvents = verifyFullExReceipt.events;

        const verifiedEvent3 = verifyEvents.pop().args;

        expect(verifiedEvent3.blockNumber).equal(3);

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(2);

        v = await franklinDeployedContract.completeWithdrawals(1);
        await v.wait();
        const afterFullExitBalance = await wallet.getBalance();

        expect(afterFullExitBalance.sub(beforeFullExitBalance)).eq(fullExitMinusGas); // full exit amount minus gas fee for send transaction

        console.log("Full exit verified");

        // Withdraw accumulated fees eth for wallet
        const accumFees = parseEther("0.006282");
        const oldBalance = await wallet.getBalance();
        const balanceToWithdraw = await franklinDeployedContract.balancesToWithdraw(wallet.address, 0);
        const exitTx = await franklinDeployedContract.withdrawETH(balanceToWithdraw);
        const exitTxReceipt = await exitTx.wait();
        const gasUsed = exitTxReceipt.gasUsed.mul(await provider.getGasPrice());
        const newBalance = await wallet.getBalance();
        expect(newBalance.sub(oldBalance).add(gasUsed)).eq(accumFees);
        expect(await franklinDeployedContract.balancesToWithdraw(wallet.address, 0)).equal(bigNumberify(0));

        console.log("Withdrawed to wallet");

        console.log(" + ETH Integration passed")
    });

    it("ERC20 deposit, part exit, full exit, commit, verify, withdraw", async () => {
        console.log("\n - ERC20 Integration started");

        const tokenId = 1;
        const tokenAddr = erc20DeployedToken.address;

        // Deposit eth
        const depositValue = 78; // the value passed to tx
        const feeValue = parseEther("0.3"); // we send in tx value
        const depositFee = parseEther("0.003852"); // tx fee get from fee value
        await erc20DeployedToken.approve(franklinDeployedContract.address, depositValue);

        const depositTx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, depositValue, franklinAddressBinary, { value: feeValue, gasLimit: bigNumberify("500000") });
        const depositReceipt = await depositTx.wait();
        const depositEvent = depositReceipt.events[3].args;

        expect(depositEvent.owner).equal(wallet.address);
        expect(depositEvent.tokenId).equal(1);
        expect(depositEvent.amount).equal(depositValue);
        expect(depositEvent.fee).equal(depositFee)
        expect(depositEvent.franklinAddress).equal("0x0809101112131415161718192021222334252627");

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(1);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(0);

        console.log("Requested deposit");

        // Commit block with eth deposit
        const depositBlockPublicData = createDepositPublicData(1, hexlify(depositValue), franklinAddress);
        const commitTx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        const commitReceipt = await commitTx.wait();
        const commitEvents = commitReceipt.events;

        const commitedEvent1 = commitEvents[0].args;

        expect(commitedEvent1.blockNumber).equal(1);

        expect(await franklinDeployedContract.totalOnchainOps()).equal(1);

        expect((await franklinDeployedContract.blocks(1)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).commitment).equal("0x7d7043f2983872e7d5632d181b0a8e0308c921b4e12ac24d69eb49def9a67c33");
        expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(1)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Deposit committed");

        // Commit block with eth partial exit.
        const exitValue = 2;
        const exitBlockPublicData = createWithdrawPublicData(1, hexlify(exitValue), exitWallet.address);

        const partExTx = await franklinDeployedContract.commitBlock(2, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            exitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        const partExReceipt = await partExTx.wait();
        const partExEvents = partExReceipt.events;

        const commitedEvent2 = partExEvents.pop().args;

        expect(commitedEvent2.blockNumber).equal(2);

        expect(await franklinDeployedContract.totalOnchainOps()).equal(2);

        expect((await franklinDeployedContract.blocks(2)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(2)).priorityOperations).equal(0);
        expect((await franklinDeployedContract.blocks(2)).commitment).equal("0xec9702b125356faae38041a7fde0094af09f2f60997f3148a86217999f1221ea");
        expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(2)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Partial exit committed");

        // Verify block with deposit and partial exit.
        const verifyDepTx = await franklinDeployedContract.verifyBlock(1, dummyBlockProof, { gasLimit: bigNumberify("500000") });
        const verifyDepReceipt = await verifyDepTx.wait();
        const verifyDepEvents = verifyDepReceipt.events;

        const verifiedEvent1 = verifyDepEvents.pop().args;

        expect(verifiedEvent1.blockNumber).equal(1);

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(1);

        console.log("Verified deposit");

        const oldBalance1 = await erc20DeployedToken.balanceOf(exitWallet.address);

        const verifyPartExTx = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, { gasLimit: bigNumberify("500000") });
        const verifyPartExReceipt = await verifyPartExTx.wait();
        const verifyPartExEvents = verifyPartExReceipt.events;

        console.log("verifyPartExEvents:", verifyPartExEvents);

        const verifiedEvent2 = verifyPartExEvents.pop().args;

        expect(verifiedEvent2.blockNumber).equal(2);

        let v = await franklinDeployedContract.completeWithdrawals(1);
        await v.wait();
        const newBalance1 = await erc20DeployedToken.balanceOf(exitWallet.address);

        expect(newBalance1.sub(oldBalance1)).eq(exitValue);

        console.log("Verified partial exit");

        // Full exit erc
        const fullExitAmount = 76; // amount after: tx value - some counted fee - exit amount
        const accId = 0;
        const pubkey = "0x0000000000000000000000000000000000000000000000000000000000000000";
        const signature = Buffer.from("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "hex");
        const nonce = 0;
        const fullExTx = await franklinDeployedContract.fullExit(
            accId,
            pubkey,
            tokenAddr,
            signature,
            nonce,
            { value: feeValue, gasLimit: bigNumberify("500000") }
        );
        await fullExTx.wait();

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(1);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(1);

        console.log("Full exit requested");

        // Commit block with full exit
        const fullExitBlockPublicData = createFullExitPublicData(
            accId, wallet.address, tokenId, hexlify(fullExitAmount)
        );
        const commitFullExTx = await franklinDeployedContract.commitBlock(3, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            fullExitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        const commitFullExReceipt = await commitFullExTx.wait();
        const commitFullExEvents = commitFullExReceipt.events;

        const commitedEvent3 = commitFullExEvents[0].args;

        expect(commitedEvent3.blockNumber).equal(3);

        expect(await franklinDeployedContract.totalOnchainOps()).equal(3);

        expect((await franklinDeployedContract.blocks(3)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).commitment).equal("0x10a7e3614ba95ff093b826f78886f190a26bd16129faaec145ffbf78d3cfdf5e");
        expect((await franklinDeployedContract.blocks(3)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(3)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Full exit committed");

        // Verify block with full exit.
        const oldBalance2 = await erc20DeployedToken.balanceOf(wallet.address);

        const verifyFullExTx = await franklinDeployedContract.verifyBlock(3, dummyBlockProof, { gasLimit: bigNumberify("500000") });
        const verifyFullExReceipt = await verifyFullExTx.wait();
        const verifyEvents = verifyFullExReceipt.events;

        const verifiedEvent3 = verifyEvents.pop().args;

        expect(verifiedEvent3.blockNumber).equal(3);

        expect(await priorityQueueDeployedContract.totalOpenPriorityRequests()).equal(0);
        expect(await priorityQueueDeployedContract.firstPriorityRequestId()).equal(2);

        v = await franklinDeployedContract.completeWithdrawals(1);
        await v.wait();

        const newBalance2 = await erc20DeployedToken.balanceOf(wallet.address);

        expect(newBalance2.sub(oldBalance2)).eq(fullExitAmount);

        console.log("Full exit verified");

        console.log(" + ERC20 Integration passed");
    });
});
