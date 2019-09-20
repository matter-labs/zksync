import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, deployGovernance} from "./deploy";

import {expect, use} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther, hexlify} from "ethers/utils";
import {createDepositPublicData, createWithdrawPublicData, createFullExitPublicData} from "./helpers"

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "0809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("INTEGRATION", function() {
    this.timeout(50000);

    let franklinDeployedContract;
    let governanceDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        console.log("---\n");
        governanceDeployedContract = await deployGovernance(wallet, wallet.address);
        franklinDeployedContract = await deployFranklin(wallet, governanceDeployedContract.address);
        erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
        // Make sure that exit wallet can execute transactions.
        await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("ETH deposit, part exit, full exit, commit, verify, withdraw", async () => {
        console.log("\n - ETH Integration started");

        // Deposit eth
        const depositValue = parseEther("0.3"); // the value passed to tx
        const depositAmount = parseEther("0.293856636"); // amount after: tx value - some counted fee
        let tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: depositValue});
        let receipt = await tx.wait();
        let events = receipt.events;

        const priorityEvent = events[0].args;

        expect(priorityEvent.opType).equal(1);
        expect(priorityEvent.pubData).equal("0x52312ad6f01657413b2eae9287f6b9adad93d5fe000000000000000000000413fd0db3abd8000809101112131415161718192021222334252627");
        expect(priorityEvent.fee).equal(bigNumberify("0x15d35b64f22800"));
        expect(priorityEvent.serialId).equal(bigNumberify(0));

        let totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        let firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(1);
        expect(firstPriorityRequestId).equal(0);

        console.log("Requested deposit");

        // Commit block with eth deposit
        const depositBlockPublicData = createDepositPublicData(0, hexlify(depositAmount), franklinAddress);
        tx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent1 = events[0].args;
        
        expect(commitedEvent1.blockNumber).equal(1);
        
        let totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(1);
        
        expect((await franklinDeployedContract.blocks(1)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).commitment).equal("0x7e59b3f061447565f803887ee8460ab3b8865b24ce34eaa991ef2c7b85cb1a40");
        expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(1)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");
        
        console.log("Deposit committed");

        // Commit block with eth partial exit.
        const exitValue = parseEther("0.2");
        const exitBlockPublicData = createWithdrawPublicData(0, hexlify(exitValue), exitWallet.address);

        tx = await franklinDeployedContract.commitBlock(2, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            exitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent2 = events.pop().args;

        expect(commitedEvent2.blockNumber).equal(2);

        totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(2);
        
        expect((await franklinDeployedContract.blocks(2)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(2)).priorityOperations).equal(0);
        expect((await franklinDeployedContract.blocks(2)).commitment).equal("0xebea7f6ebc71aeb2febfbd750ec46f513d1e527c2bf5a98d7f65e3bbbb285dcb");
        expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(2)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");
        
        console.log("Partial exit committed");

        // Verify block with deposit and partial exit.
        tx = await franklinDeployedContract.verifyBlock(1, dummyBlockProof, {gasLimit: bigNumberify("500000")});
        receipt = await tx.wait();
        events = receipt.events;
        
        const verifiedEvent1 = events.pop().args;

        expect(verifiedEvent1.blockNumber).equal(1);
        
        totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(0);
        expect(firstPriorityRequestId).equal(1);

        console.log("Verified deposit");
        
        tx = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, {gasLimit: bigNumberify("500000")});
        receipt = await tx.wait();
        events = receipt.events;

        const verifiedEvent2 = events.pop().args;

        expect(verifiedEvent2.blockNumber).equal(2);
        
        let balanceToWithdraw1 = await franklinDeployedContract.balancesToWithdraw(exitWallet.address, 0);
        expect(balanceToWithdraw1).equal(exitValue);

        console.log("Verified partial exit");

        // Full exit eth
        const fullExitAmount = parseEther("0.093856636"); // amount after: tx value - some counted fee - exit amount
        tx = await franklinDeployedContract.fullExit(
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "0x0000000000000000000000000000000000000000",
            Buffer.from("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "hex"),
            0,
            {value: depositValue, gasLimit: bigNumberify("500000")}
        );
        receipt = await tx.wait();
        events = receipt.events;

        const fullExitEvent = events[0].args;
        expect(fullExitEvent.opType).equal(6);
        expect(fullExitEvent.pubData).equal("0x000000000000000000000000000000000000000000000000000000000000000052312ad6f01657413b2eae9287f6b9adad93d5fe00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
        expect(fullExitEvent.fee).equal(bigNumberify("0x3fa4e182b0e800"));
        expect(fullExitEvent.serialId).equal(bigNumberify(1));

        totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(1);
        expect(firstPriorityRequestId).equal(1);

        console.log("Full exit requested");

        // Commit block with full exit
        const fullExitBlockPublicData = createFullExitPublicData(wallet.address, 0, hexlify(fullExitAmount));
        tx = await franklinDeployedContract.commitBlock(3, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            fullExitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent3 = events[0].args;
        
        expect(commitedEvent3.blockNumber).equal(3);
        
        totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(3);
        
        expect((await franklinDeployedContract.blocks(3)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).commitment).equal("0x0076b81f61e5c1f27d3c50afb0a43897d0b2178a28a6bb63ad3130ad3f3c65bc");
        expect((await franklinDeployedContract.blocks(3)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(3)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");
        
        console.log("Full exit committed");

        // Verify block with full exit.
        tx = await franklinDeployedContract.verifyBlock(3, dummyBlockProof, {gasLimit: bigNumberify("500000")});
        receipt = await tx.wait();
        events = receipt.events;
        
        const verifiedEvent3 = events.pop().args;

        expect(verifiedEvent3.blockNumber).equal(3);
        
        totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(0);
        expect(firstPriorityRequestId).equal(2);
        
        let balanceToWithdraw2 = await franklinDeployedContract.balancesToWithdraw(wallet.address, 0);
        expect(balanceToWithdraw2).equal("0x01A2EA59E03AE800"); // amount - part exit + fee

        console.log("Full exit verified");

        // Withdraw eth for wallet
        const oldBalance2 = await wallet.getBalance();
        const exitTx2 = await franklinDeployedContract.withdrawETH(balanceToWithdraw2);
        const exitTxReceipt2 = await exitTx2.wait();
        const gasUsed2 = exitTxReceipt2.gasUsed.mul(await provider.getGasPrice());
        const newBalance2 = await wallet.getBalance();
        expect(newBalance2.sub(oldBalance2).add(gasUsed2)).eq(balanceToWithdraw2);

        balanceToWithdraw2 = await franklinDeployedContract.balancesToWithdraw(wallet.address, 0);
        expect(balanceToWithdraw2).equal(bigNumberify(0));

        console.log("Withdrawed to 1st wallet");

        // Withdraw eth for exitWallet
        const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);
        const oldBalance1 = await exitWallet.getBalance();
        const exitTx1 = await exitWalletFranklinContract.withdrawETH(balanceToWithdraw1, {gasLimit: bigNumberify("500000")});
        const exitTxReceipt1 = await exitTx1.wait();
        const gasUsed1 = exitTxReceipt1.gasUsed.mul(await provider.getGasPrice());
        const newBalance1 = await exitWallet.getBalance();
        expect(newBalance1.sub(oldBalance1).add(gasUsed1)).eq(balanceToWithdraw1);

        balanceToWithdraw1 = await exitWalletFranklinContract.balancesToWithdraw(exitWallet.address, 0);
        expect(balanceToWithdraw1).equal(bigNumberify(0));

        console.log("Withdrawed to 2nd wallet");

        console.log(" + ETH Integration passed")
    });

    it("ERC20 deposit, part exit, full exit, commit, verify, withdraw", async () => {
        console.log("\n - ERC20 Integration started");
        
        // erc deposit
        const depositValue = 78;
        const feeValue = parseEther("0.3");
        await erc20DeployedToken.approve(franklinDeployedContract.address, depositValue);

        let tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, depositValue, franklinAddressBinary, {value: feeValue, gasLimit: bigNumberify("500000")});
        let receipt = await tx.wait();
        let events = receipt.events;

        const priorityEvent = events[2].args;

        expect(priorityEvent.opType).equal(1);
        expect(priorityEvent.pubData).equal("0x52312ad6f01657413b2eae9287f6b9adad93d5fe00010000000000000000000000000000004e0809101112131415161718192021222334252627");
        expect(priorityEvent.fee).equal(bigNumberify("0x3f69b76956d000"));
        expect(priorityEvent.serialId).equal(bigNumberify(0));

        let totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        let firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(1);
        expect(firstPriorityRequestId).equal(0);

        console.log("Requested deposit");

        // Commit block with erc deposit
        const depositBlockPublicData = createDepositPublicData(1, hexlify(depositValue), franklinAddress);
        tx = await franklinDeployedContract.commitBlock(1, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            depositBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent1 = events[0].args;
        
        expect(commitedEvent1.blockNumber).equal(1);
        
        let totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(1);
        
        expect((await franklinDeployedContract.blocks(1)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(1)).commitment).equal("0x7d7043f2983872e7d5632d181b0a8e0308c921b4e12ac24d69eb49def9a67c33");
        expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(1)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        console.log("Deposit committed");

        // Commit block with erc partial exit.
        const exitValue = 2;
        const exitBlockPublicData = createWithdrawPublicData(1, hexlify(exitValue), exitWallet.address);

        tx = await franklinDeployedContract.commitBlock(2, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            exitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );
        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent2 = events.pop().args;

        expect(commitedEvent2.blockNumber).equal(2);

        totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(2);
        
        expect((await franklinDeployedContract.blocks(2)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(2)).priorityOperations).equal(0);
        expect((await franklinDeployedContract.blocks(2)).commitment).equal("0xec9702b125356faae38041a7fde0094af09f2f60997f3148a86217999f1221ea");
        expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(2)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");
        
        console.log("Partial exit committed");

        // Verify block with deposit and exit.
        tx = await franklinDeployedContract.verifyBlock(1, dummyBlockProof, {gasLimit: bigNumberify("500000")});
        receipt = await tx.wait();
        events = receipt.events;
        
        const verifiedEvent1 = events.pop().args;

        expect(verifiedEvent1.blockNumber).equal(1);
        
        totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(0);
        expect(firstPriorityRequestId).equal(1);

        console.log("Verified deposit");
        
        tx = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, {gasLimit: bigNumberify("500000")});
        receipt = await tx.wait();
        events = receipt.events;

        const verifiedEvent2 = events.pop().args;

        expect(verifiedEvent2.blockNumber).equal(2);
        
        let balanceToWithdraw1 = await franklinDeployedContract.balancesToWithdraw(exitWallet.address, 1);
        expect(balanceToWithdraw1).equal(exitValue);

        console.log("Verified partial exit");

        // Full exit erc
        const fullExitAmount = 76; // amount after: tx value - some counted fee - exit amount
        tx = await franklinDeployedContract.fullExit(
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            erc20DeployedToken.address,
            Buffer.from("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "hex"),
            0,
            {value: feeValue, gasLimit: bigNumberify("500000")}
        );
        receipt = await tx.wait();
        events = receipt.events;

        const fullExitEvent = events[0].args;
        expect(fullExitEvent.opType).equal(6);
        expect(fullExitEvent.pubData).equal("0x000000000000000000000000000000000000000000000000000000000000000052312ad6f01657413b2eae9287f6b9adad93d5fe00010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
        expect(fullExitEvent.fee).equal(bigNumberify("0x3f7af8aca8e800"));
        expect(fullExitEvent.serialId).equal(bigNumberify(1));

        totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(1);
        expect(firstPriorityRequestId).equal(1);

        console.log("Full exit requested");

        // Commit block with full exit
        const fullExitBlockPublicData = createFullExitPublicData(wallet.address, 1, hexlify(fullExitAmount));
        tx = await franklinDeployedContract.commitBlock(3, 22,
            Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
            fullExitBlockPublicData,
            {
                gasLimit: bigNumberify("500000"),
            },
        );

        receipt = await tx.wait();
        events = receipt.events;

        const commitedEvent3 = events[0].args;
        
        expect(commitedEvent3.blockNumber).equal(3);
        
        totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(3);
        
        expect((await franklinDeployedContract.blocks(3)).onchainOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).priorityOperations).equal(1);
        expect((await franklinDeployedContract.blocks(3)).commitment).equal("0xb793fbd68a0da3464368a0c40701115dd08ec6a994a5822953985226848c5a59");
        expect((await franklinDeployedContract.blocks(3)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(3)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");
            
        console.log("Full exit committed");

        // Verify block with full exit.
        tx = await franklinDeployedContract.verifyBlock(3, dummyBlockProof, {gasLimit: bigNumberify("500000")});
        receipt = await tx.wait();
        events = receipt.events;
        
        const verifiedEvent3 = events.pop().args;

        expect(verifiedEvent3.blockNumber).equal(3);
        
        totalOpenPriorityRequests = await franklinDeployedContract.totalOpenPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalOpenPriorityRequests).equal(0);
        expect(firstPriorityRequestId).equal(2);
        
        let balanceToWithdraw2 = await franklinDeployedContract.balancesToWithdraw(wallet.address, 1);
        expect(balanceToWithdraw2).equal(fullExitAmount);

        console.log("Full exit verified");

        // Withdraw erc20 for wallet
        const oldBalance2 = await erc20DeployedToken.balanceOf(wallet.address);
        const exitTx2 = await franklinDeployedContract.withdrawERC20(erc20DeployedToken.address, balanceToWithdraw2);
        await exitTx2.wait();
        const newBalance2 = await erc20DeployedToken.balanceOf(wallet.address);
        expect(newBalance2.sub(oldBalance2)).eq(balanceToWithdraw2);
        balanceToWithdraw2 = await franklinDeployedContract.balancesToWithdraw(wallet.address, 1);
        expect(balanceToWithdraw2).equal(bigNumberify(0));

        console.log("Withdrawed to 1st wallet");

        // Withdraw erc20 for exit wallet
        const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);
        const oldBalance1 = await erc20DeployedToken.balanceOf(exitWallet.address);
        const exitTx1 = await exitWalletFranklinContract.withdrawERC20(erc20DeployedToken.address, balanceToWithdraw1);
        await exitTx1.wait();
        const newBalance1 = await erc20DeployedToken.balanceOf(exitWallet.address);
        expect(newBalance1.sub(oldBalance1)).eq(balanceToWithdraw1);
        balanceToWithdraw1 = await exitWalletFranklinContract.balancesToWithdraw(exitWallet.address, 1);
        expect(balanceToWithdraw1).equal(bigNumberify(0));

        console.log("Withdrawed to 2nd wallet");

        console.log(" + ERC20 Integration passed")
    });
});
