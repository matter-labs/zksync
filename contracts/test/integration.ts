import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, deployGovernance} from "../src.ts/deploy";

import {expect, use} from "chai";
import {solidity} from "ethereum-waffle";
import {bigNumberify, parseEther, hexlify} from "ethers/utils";
import {createDepositPublicData, createWithdrawPublicData} from "./helpers"

use(solidity);

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);
const franklinAddress = "0809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");
const dummyBlockProof = [0, 0, 0, 0, 0, 0, 0, 0];

describe("INTEGRATION: Complete", function() {
    this.timeout(30000);

    let franklinDeployedContract;
    let governanceDeployedContract;
    let erc20DeployedToken;

    beforeEach(async () => {
        if (governanceDeployedContract == null || franklinDeployedContract == null || erc20DeployedToken == null) {
            governanceDeployedContract = await deployGovernance(wallet, wallet.address);
            franklinDeployedContract = await deployFranklin(wallet, governanceDeployedContract.address);
            erc20DeployedToken = await addTestERC20Token(wallet, governanceDeployedContract);
        }
        // Make sure that exit wallet can execute transactions.
        await wallet.sendTransaction({to: exitWallet.address, value: parseEther("1.0")});
    });

    it("Ether deposit, commit, withdraw, verify", async () => {
        // Deposit eth
        const depositValue = parseEther("0.3"); // the value passed to tx
        const depositAmount = parseEther("0.293775600000000000"); // amount after: tx value - some counted fee
        let tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: depositValue});
        let receipt = await tx.wait();
        let events = receipt.events;

        const priorityEvent = events[0].args;
        const depositEvent = events[1].args;

        expect(priorityEvent.opType).equal(1);
        expect(priorityEvent.pubData).equal("0x52312ad6f01657413b2eae9287f6b9adad93d5fe000000000000000000000413b35a09ad60000809101112131415161718192021222334252627");
        expect(priorityEvent.fee).equal(bigNumberify("0x161d0f0ef0a000"));

        expect(depositEvent.owner).equal(wallet.address);
        expect(depositEvent.tokenId).equal(0);
        expect(depositEvent.amount).equal(depositAmount);
        expect(depositEvent.franklinAddress).equal("0x" + franklinAddress);

        let totalPriorityRequests = await franklinDeployedContract.totalPriorityRequests();
        let firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalPriorityRequests).equal(1);
        expect(firstPriorityRequestId).equal(0);

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
        expect((await franklinDeployedContract.blocks(1)).commitment).equal("0x7b4e12b6adec3e0f9e317e1575d399ef8ed67a7ae798224371afc33e2a0fed81");
        expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(1)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

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
        expect((await franklinDeployedContract.blocks(2)).commitment).equal("0xf7064daefcb240b84ce4f0d8b88d7dcb1cb8aaaf24ccf8a1b9bc42faabc24f15");
        expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x0000000000000000000000000000000000000000000000000000000000000000");
        expect((await franklinDeployedContract.blocks(2)).validator).equal("0x52312AD6f01657413b2eaE9287f6B9ADaD93D5FE");

        // Verify block with deposit and exit.
        tx = await franklinDeployedContract.verifyBlock(1, dummyBlockProof, {gasLimit: bigNumberify("100000")});
        receipt = await tx.wait();
        events = receipt.events;
        
        const verifiedEvent1 = events.pop().args;

        expect(verifiedEvent1.blockNumber).equal(1);

        totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(1);
        
        totalPriorityRequests = await franklinDeployedContract.totalPriorityRequests();
        firstPriorityRequestId = await franklinDeployedContract.firstPriorityRequestId();
        expect(totalPriorityRequests).equal(0);
        expect(firstPriorityRequestId).equal(1);
        
        tx = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, {gasLimit: bigNumberify("100000")});
        receipt = await tx.wait();
        events = receipt.events;

        const verifiedEvent2 = events.pop().args;

        expect(verifiedEvent2.blockNumber).equal(2);

        totalOnchainOps = await franklinDeployedContract.totalOnchainOps();
        expect(totalOnchainOps).equal(0);

        expect((await franklinDeployedContract.balancesToWithdraw(exitWallet.address, 0)).balance).equal(exitValue);

        // Withdraw eth.
        const oldBalance = await exitWallet.getBalance();
        const exitTx = await franklinDeployedContract.withdrawETH(exitValue);
        const exitTxReceipt = await exitTx.wait();
        const gasUsed = exitTxReceipt.gasUsed.mul(await provider.getGasPrice());
        const newBalance = await exitWallet.getBalance();
        expect(newBalance.sub(oldBalance).add(gasUsed)).eq(exitValue);
        expect((await franklinDeployedContract.balancesToWithdraw(exitWallet.address, 0)).balance).equal(bigNumberify(0));
    });

    // it("ERC20 deposit with commit", async () => {
    //     const depositValue = bigNumberify("78");
    //     const depositFee = bigNumberify("8");
    //     await erc20DeployedToken.approve(franklinDeployedContract.address, depositValue);

    //     let tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, depositValue, franklinAddressBinary,
    //         {gasLimit: bigNumberify("150000"), value: depositFee});
    //     let receipt = await tx.wait();
    //     let events = receipt.events;

    //     const priorityEvent = events[0].pop().args;
    //     const depositEvent = events[1].pop().args;

    //     expect(priorityEvent.opType).equal(1);
    //     expect(priorityEvent.pubData).equal("0x");
    //     expect(priorityEvent.expirationBlock).equal(bigNumberify(0));
    //     expect(priorityEvent.fee).equal(bigNumberify(8));

    //     expect(depositEvent.owner).equal(wallet.address);
    //     expect(depositEvent.tokenId).equal(1);
    //     expect(depositEvent.amount).equal(depositValue);
    //     expect(depositEvent.franklinAddress).equal("0x" + franklinAddress);

    //     expect(await franklinDeployedContract.priorityRequests(0)).equal("0x");
    //     expect(await franklinDeployedContract.firstPriorityRequestId).equal(0);
    //     expect(await franklinDeployedContract.totalPriorityRequests).equal(1);

    //     // Commit block with erc deposit
    //     const depositBlockPublicData = createDepositPublicData(1, depositValue, franklinAddress);
    //     tx = await franklinDeployedContract.commitBlock(1, 22,
    //         Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
    //         depositBlockPublicData,
    //         {
    //             gasLimit: bigNumberify("500000"),
    //         },
    //     );
    //     receipt = await tx.wait();
    //     events = receipt.events;

    //     const commitedEvent1 = events.pop().args;
        
    //     expect(commitedEvent1.blockNumber).equal(1);

    //     expect(await franklinDeployedContract.onchainOps(0)).equal("0x");
    //     expect(await franklinDeployedContract.totalOnchainOps).equal(1);
        
    //     expect((await franklinDeployedContract.blocks(1)).onchainOperations).equal(1);
    //     expect((await franklinDeployedContract.blocks(1)).priorityOperations).equal(1);
    //     expect((await franklinDeployedContract.blocks(1)).commitment).equal("0x");
    //     expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x");
    //     expect((await franklinDeployedContract.blocks(1)).stateRoot).equal("0x");

    //     // Commit block with erc partial exit.
    //     const exitValue = bigNumberify("45");
    //     const exitBlockPublicData = createWithdrawPublicData(1, exitValue, exitWallet.address);
    //     tx = await franklinDeployedContract.commitBlock(2, 22,
    //         Buffer.from("0000000000000000000000000000000000000000000000000000000000000000", "hex"),
    //         exitBlockPublicData,
    //         {
    //             gasLimit: bigNumberify("500000"),
    //         },
    //     );
    //     receipt = await tx.wait();
    //     events = receipt.events;

    //     const commitedEvent2 = events.pop().args;

    //     expect(commitedEvent2.blockNumber).equal(2);

    //     expect(await franklinDeployedContract.onchainOps(1)).equal("0x");
    //     expect(await franklinDeployedContract.totalOnchainOps).equal(2);
        
    //     expect((await franklinDeployedContract.blocks(2)).onchainOperations).equal(1);
    //     expect((await franklinDeployedContract.blocks(2)).priorityOperations).equal(0);
    //     expect((await franklinDeployedContract.blocks(2)).commitment).equal("0x");
    //     expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x");
    //     expect((await franklinDeployedContract.blocks(2)).stateRoot).equal("0x");

    //     // Verify block with deposit and exit.
    //     tx = await franklinDeployedContract.verifyBlock(1, dummyBlockProof, {gasLimit: bigNumberify("100000")});
    //     receipt = await tx.wait();
    //     events = receipt.events;
        
    //     const verifiedEvent1 = events.pop().args;

    //     expect(verifiedEvent1.blockNumber).equal(1);

    //     expect(await franklinDeployedContract.onchainOps(0)).equal("0x");
    //     expect(await franklinDeployedContract.totalOnchainOps).equal(1);
        
    //     expect(await franklinDeployedContract.priorityRequests(0)).equal("0x");
    //     expect(await franklinDeployedContract.firstPriorityRequestId).equal(0);
    //     expect(await franklinDeployedContract.totalPriorityRequests).equal(0);
        
    //     tx = await franklinDeployedContract.verifyBlock(2, dummyBlockProof, {gasLimit: bigNumberify("100000")});
    //     receipt = await tx.wait();
    //     events = receipt.events;

    //     const verifiedEvent2 = events.pop().args;

    //     expect(verifiedEvent2.blockNumber).equal(1);

    //     expect(await franklinDeployedContract.onchainOps(1)).equal("0x");
    //     expect(await franklinDeployedContract.totalOnchainOps).equal(0);

    //     expect((await franklinDeployedContract.balancesToWithdraw(exitWallet.address, 1)).balance).equal(exitValue);

    //     // Withdraw erc20.
    //     const exitWalletFranklinContract = franklinDeployedContract.connect(exitWallet);
    //     const exitTx = await exitWalletFranklinContract.withdrawERC20(erc20DeployedToken.address, exitValue);
    //     const recp = await exitTx.wait();
    //     expect(await erc20DeployedToken.balanceOf(exitWallet.address)).eq(exitValue);
    //     expect((await franklinDeployedContract.balancesToWithdraw(exitWallet.address, 1)).balance).equal(bigNumberify(0));
    // });
});
