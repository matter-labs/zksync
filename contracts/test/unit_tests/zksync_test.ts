import {
    addTestERC20Token, addTestNotApprovedERC20Token,
    franklinTestContractCode,
    governanceTestContractCode, mintTestERC20Token,
    verifierTestContractCode, Deployer
} from "../../src.ts/deploy";
import {BigNumber, bigNumberify, BigNumberish, parseEther} from "ethers/utils";
import {ETHProxy} from "zksync";
import {Address, TokenAddress} from "zksync/build/types";
import {AddressZero} from "ethers/constants";
import {Contract, ethers} from "ethers";

const abi = require('ethereumjs-abi')
const {expect} = require("chai")
const {deployContract} = require("ethereum-waffle");
const {wallet, exitWallet, deployTestContract, getCallRevertReason, IERC20_INTERFACE} = require("./common");
import * as zksync from "zksync";

const TEST_PRIORITY_EXPIRATION = 16;


describe("zkSync signature verification unit tests", function () {
    this.timeout(50000);

    let testContract;
    let randomWallet = ethers.Wallet.createRandom();
    before(async () => {
        const deployer = new Deployer(wallet, true);
        await deployer.deployGovernance();
        await deployer.deployVerifier();
        process.env.OPERATOR_FRANKLIN_ADDRESS = wallet.address;
        deployer.bytecodes.FranklinTarget = require("../../build/ZKSyncUnitTest");
        testContract = await deployer.deployFranklin();
    });

    it("pubkey hash signature verification success", async () => {
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {revertReason, result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), nonce, randomWallet.address));
        expect(result).eq(true);
    });

    it("pubkey hash signature verification incorrect nonce", async () => {
        const incorrectNonce = 0x11223345;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), incorrectNonce, randomWallet.address));
        expect(result).eq(false);
    });

    it("pubkey hash signature verification incorrect pubkey hash", async () => {
        const incorrectPubkeyHash = "sync:aaaafefefefefefefefefefefefefefefefefefe";
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, incorrectPubkeyHash.replace("sync:", "0x"), nonce, randomWallet.address));
        expect(result).eq(false);
    });

    it("pubkey hash signature verification incorrect signer", async () => {
        const incorrectSignerAddress = wallet.address;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), nonce, incorrectSignerAddress));
        expect(result).eq(false);
    });

    it("signature verification success", async () => {
        for (const message of [Buffer.from("msg", "ascii"), Buffer.alloc(0), Buffer.alloc(10, 1)]) {
            const signature = await wallet.signMessage(message);
            const sinedMessage = Buffer.concat([Buffer.from(`\x19Ethereum Signed Message:\n${message.length}`, "ascii"), message]);
            const address = await testContract.testVerifyEthereumSignature(signature, sinedMessage);
            expect(address, `address mismatch, message ${message.toString("hex")}`).eq(wallet.address);
        }
    });
});

describe("ZK priority queue ops unit tests", function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let operationTestContract;
    before(async () => {
        const deployer = new Deployer(wallet, true);
        const governanceDeployedContract = await deployer.deployGovernance();
        await deployer.deployVerifier();
        process.env.OPERATOR_FRANKLIN_ADDRESS = wallet.address;
        zksyncContract = await deployer.deployFranklin();
        await governanceDeployedContract.setValidator(wallet.address, true);
        tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, tokenContract);
        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: governanceDeployedContract.address
        });

        operationTestContract = await deployTestContract('../../build/OperationsTest');
    });

    async function performDeposit(to: Address, token: TokenAddress, depositAmount: BigNumber) {
        const openedRequests = await zksyncContract.totalOpenPriorityRequests();
        const depositOwner = wallet.address;

        let tx;
        if (token === ethers.constants.AddressZero) {
            tx = await zksyncContract.depositETH(depositOwner, {value: depositAmount});
        } else {
            tx = await zksyncContract.depositERC20(token, depositAmount, depositOwner);
        }
        const receipt = await tx.wait();

        const deadlineBlock = receipt.blockNumber + TEST_PRIORITY_EXPIRATION;

        let priorityQueueEvent;
        for (const event of receipt.logs) {
            const parsedLog = zksyncContract.interface.parseLog(event);
            if (parsedLog && parsedLog.name === "NewPriorityRequest") {
                priorityQueueEvent = parsedLog;
                break;
            }
        }
        expect(priorityQueueEvent.name, "event name").eq("NewPriorityRequest");
        expect(priorityQueueEvent.values.sender, "sender address").eq(wallet.address);
        expect(priorityQueueEvent.values.serialId, "request id").eq(openedRequests);
        expect(priorityQueueEvent.values.opType, "request type").eq(1);
        expect(priorityQueueEvent.values.expirationBlock, "expiration block").eq(deadlineBlock);
        const parsedDepositPubdata = await operationTestContract.parseDepositFromPubdata(priorityQueueEvent.values.pubData);

        expect(parsedDepositPubdata.tokenId, "parsed token id").eq(await ethProxy.resolveTokenId(token));
        expect(parsedDepositPubdata.amount.toString(), "parsed amount").eq(depositAmount.toString());
        expect(parsedDepositPubdata.owner, "parsed owner").eq(depositOwner);
    }

    async function performFullExitRequest(accountId: number, token: TokenAddress) {
        const openedRequests = await zksyncContract.totalOpenPriorityRequests();
        const tx = await zksyncContract.fullExit(accountId, token);
        const receipt = await tx.wait();

        const deadlineBlock = receipt.blockNumber + TEST_PRIORITY_EXPIRATION;

        let priorityQueueEvent;
        for (const event of receipt.logs) {
            const parsedLog = zksyncContract.interface.parseLog(event);
            if (parsedLog && parsedLog.name === "NewPriorityRequest") {
                priorityQueueEvent = parsedLog;
                break;
            }
        }
        expect(priorityQueueEvent.name, "event name").eq("NewPriorityRequest");
        expect(priorityQueueEvent.values.sender, "sender address").eq(wallet.address);
        expect(priorityQueueEvent.values.serialId, "request id").eq(openedRequests);
        expect(priorityQueueEvent.values.opType, "request type").eq(6);
        expect(priorityQueueEvent.values.expirationBlock, "expiration block").eq(deadlineBlock);

        const parsedFullExitPubdata = await operationTestContract.parseFullExitFromPubdata(priorityQueueEvent.values.pubData);
        expect(parsedFullExitPubdata.accountId, "parsed account id").eq(accountId);
        expect(parsedFullExitPubdata.owner, "parsed owner").eq(wallet.address);
        expect(parsedFullExitPubdata.tokenId, "parsed token id").eq(await ethProxy.resolveTokenId(token));
        expect(parsedFullExitPubdata.amount.toString(), "parsed amount").eq("0");
    }

    it("success ETH deposits", async () => {
        zksyncContract.connect(wallet);
        const tokenAddress = ethers.constants.AddressZero;
        const depositAmount = parseEther("1.0");

        await performDeposit(wallet.address, tokenAddress, depositAmount);
        await performDeposit(ethers.Wallet.createRandom().address, tokenAddress, depositAmount);
    });

    it("success ERC20 deposits", async () => {
        zksyncContract.connect(wallet);
        const tokenAddress = tokenContract.address;
        const depositAmount = parseEther("1.0");

        tokenContract.connect(wallet);
        await tokenContract.approve(zksyncContract.address, depositAmount);
        await performDeposit(wallet.address, tokenAddress, depositAmount);
        await tokenContract.approve(zksyncContract.address, depositAmount);
        await performDeposit(ethers.Wallet.createRandom().address, tokenAddress, depositAmount);
    });

    it("success FullExit request", async () => {
        zksyncContract.connect(wallet);
        const accountId = 1;

        await performFullExitRequest(accountId, ethers.constants.AddressZero);
        await performFullExitRequest(accountId, tokenContract.address);
    });
});

async function onchainBalance(ethWallet: ethers.Wallet, token: Address): Promise<BigNumber> {
    if (token === ethers.constants.AddressZero) {
        return ethWallet.getBalance();
    } else {
        const erc20contract = new Contract(
            token,
            IERC20_INTERFACE.abi,
            ethWallet,
        );
        return bigNumberify(await erc20contract.balanceOf(ethWallet.address));
    }
}

describe("zkSync withdraw unit tests", function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;
    before(async () => {
        const deployer = new Deployer(wallet, true);
        const governanceDeployedContract = await deployer.deployGovernance();
        await deployer.deployVerifier();
        process.env.OPERATOR_FRANKLIN_ADDRESS = wallet.address;
        deployer.bytecodes.FranklinTarget = require("../../build/ZKSyncUnitTest");
        zksyncContract = await deployer.deployFranklin();
        await governanceDeployedContract.setValidator(wallet.address, true);
        tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, tokenContract);
        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: governanceDeployedContract.address
        });

        incorrectTokenContract = await addTestNotApprovedERC20Token(wallet);
        await mintTestERC20Token(wallet, tokenContract);
    });

    async function performWithdraw(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        const contractBalanceBefore = bigNumberify((await zksyncContract.balancesToWithdraw(ethWallet.address, tokenId)).balanceToWithdraw);
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawETH(amount, {gasLimit: 70000});
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawERC20(token, amount, {gasLimit: 70000});
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance = token == AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), "withdraw account balance mismatch").eq(expectedBalance.toString());

        const contractBalanceAfter = bigNumberify((await zksyncContract.balancesToWithdraw(ethWallet.address, tokenId)).balanceToWithdraw);
        const expectedContractBalance = contractBalanceBefore.sub(amount);
        expect(contractBalanceAfter.toString(), "withdraw contract balance mismatch").eq(expectedContractBalance.toString());
    }

    it("Withdraw ETH success", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendETH = await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount.mul(2),
            data: abi.simpleEncode("receiveETH()")
        });
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        await performWithdraw(wallet, AddressZero, 0, withdrawAmount);

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        await performWithdraw(wallet, AddressZero, 0, withdrawAmount.div(2));
        await performWithdraw(wallet, AddressZero, 0, withdrawAmount.div(2));
    });

    it("Withdraw ETH incorrect ammount", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendETH = await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount,
            data: abi.simpleEncode("receiveETH()")
        });
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        const {revertReason} = await getCallRevertReason(async () => await performWithdraw(wallet, AddressZero, 0, withdrawAmount.add(1)));
        expect(revertReason, "wrong revert reason").eq("frw11");
    });

    it("Withdraw ERC20 success", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount.mul(2));
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.div(2));
        await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.div(2));
    });

    it("Withdraw ERC20 incorrect amount", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendERC20 = await tokenContract.transfer(zksyncContract.address, withdrawAmount);
        await sendERC20.wait();
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);

        await zksyncContract.setBalanceToWithdraw(wallet.address, tokenId, withdrawAmount);

        const {revertReason} = await getCallRevertReason(async () => await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.add(1)));
        expect(revertReason, "wrong revert reason").eq("frw11");
    });

    it("Withdraw ERC20 unsupported token", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const {revertReason} = await getCallRevertReason(async () => await performWithdraw(wallet, incorrectTokenContract.address, 1, withdrawAmount.add(1)));
        expect(revertReason, "wrong revert reason").eq("gvs11");
    });

    it("Complete pending withdawals, eth, known erc20", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");
        const withdrawsToCancel = 5;

        await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount,
            data: abi.simpleEncode("receiveETH()")
        });
        await tokenContract.transfer(zksyncContract.address, withdrawAmount);


        for (const tokenAddress of [AddressZero, tokenContract.address]) {
            const tokenId = await ethProxy.resolveTokenId(tokenAddress);

            await zksyncContract.setBalanceToWithdraw(exitWallet.address, tokenId, 0);
            await zksyncContract.addPendingWithdrawal(exitWallet.address, tokenId, withdrawAmount.div(2));
            await zksyncContract.addPendingWithdrawal(exitWallet.address, tokenId, withdrawAmount.div(2));

            const onchainBalBefore = await onchainBalance(exitWallet, tokenAddress);

            await zksyncContract.completeWithdrawals(withdrawsToCancel);

            const onchainBalAfter = await onchainBalance(exitWallet, tokenAddress);

            expect(onchainBalAfter.sub(onchainBalBefore)).eq(withdrawAmount.toString());
        }
    });
});

describe("zkSync auth pubkey onchain unit tests", function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    before(async () => {
        const deployer = new Deployer(wallet, true);
        const governanceDeployedContract = await deployer.deployGovernance();
        await deployer.deployVerifier();
        process.env.OPERATOR_FRANKLIN_ADDRESS = wallet.address;
        deployer.bytecodes.FranklinTarget = require("../../build/ZKSyncUnitTest");
        zksyncContract = await deployer.deployFranklin();
        await governanceDeployedContract.setValidator(wallet.address, true);
        tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, tokenContract);
        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: governanceDeployedContract.address
        });
    });

    it("Auth pubkey success", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefe";

        const receipt = await (await zksyncContract.authPubkeyHash(pubkeyHash, nonce)).wait();
        let authEvent;
        for (const event of receipt.logs) {
            const parsedLog = zksyncContract.interface.parseLog(event);
            if (parsedLog && parsedLog.name === "FactAuth") {
                authEvent = parsedLog;
                break;
            }
        }

        expect(authEvent.values.sender, "event sender incorrect").eq(wallet.address);
        expect(authEvent.values.nonce, "event nonce incorrect").eq(nonce);
        expect(authEvent.values.fact, "event fact incorrect").eq(pubkeyHash);
    });

    it("Auth pubkey rewrite fail", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0xdead;
        const pubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefe";

        await zksyncContract.authPubkeyHash(pubkeyHash, nonce);
        //
        const otherPubkeyHash = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const {revertReason} = await getCallRevertReason(async () => await zksyncContract.authPubkeyHash(otherPubkeyHash, nonce));
        expect(revertReason, "revert reason incorrect").eq("ahf11");
    });

    it("Auth pubkey incorrect length fail", async () => {
        zksyncContract.connect(wallet);
        const nonce = 0x7656;
        const shortPubkeyHash = "0xfefefefefefefefefefefefefefefefefefefe";
        const longPubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefefe";

        for (const pkHash of [shortPubkeyHash, longPubkeyHash]) {
            const {revertReason} = await getCallRevertReason(async () => await zksyncContract.authPubkeyHash(shortPubkeyHash, nonce));
            expect(revertReason, "revert reason incorrect").eq("ahf10");
        }
    });
});

describe("zkSync test process next operation", function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;
    before(async () => {
        const deployer = new Deployer(wallet, true);
        const governanceDeployedContract = await deployer.deployGovernance();
        await deployer.deployVerifier();
        process.env.OPERATOR_FRANKLIN_ADDRESS = wallet.address;
        deployer.bytecodes.FranklinTarget = require("../../build/ZKSyncUnitTest");
        zksyncContract = await deployer.deployFranklin();
        await governanceDeployedContract.setValidator(wallet.address, true);
        tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, tokenContract);
        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: governanceDeployedContract.address
        });
    });

    it("Process noop", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(8, 0);
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process transfer", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(8 * 2, 0xff);
        pubdata[0] = 0x05;
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });
    it("Process transfer to new", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(8 * 5, 0xff);
        pubdata[0] = 0x02;
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process deposit", async () => {
        zksyncContract.connect(wallet);
        const depositAmount = parseEther("0.8");

        await zksyncContract.depositETH(wallet.address, {value: depositAmount});

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(8 * 6, 0);
        pubdata[0] = 0x01;
        pubdata.writeUIntBE(0xaabbff, 1, 3);
        Buffer.from(depositAmount.toHexString().substr(2).padStart(16 * 2, "0"), "hex").copy(pubdata, 6);
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, 22);
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter - 1, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process partial exit", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(8 * 6, 0);
        pubdata[0] = 0x03;

        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process full exit", async () => {
        zksyncContract.connect(wallet);
        const tokenId = 0x01;
        const fullExitAmount = parseEther("0.7");
        const accountId = 0xaabbff;

        await zksyncContract.fullExit(accountId, tokenContract.address);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(8 * 6, 0);
        pubdata[0] = 0x06;
        pubdata.writeUIntBE(accountId, 1, 3);
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, 4);
        pubdata.writeUInt16BE(tokenId, 24);
        Buffer.from(fullExitAmount.toHexString().substr(2).padStart(16 * 2, "0"), "hex").copy(pubdata, 26);

        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter - 1, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Change pubkey with auth", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefe";
        await zksyncContract.authPubkeyHash(pubkeyHash, nonce);

        const accountId = 0xffee12;

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(8 * 6, 0);
        pubdata[0] = 0x07;
        pubdata.writeUIntBE(accountId, 1, 3);
        Buffer.from(pubkeyHash.substr(2), "hex").copy(pubdata, 4);
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, 24);
        pubdata.writeUInt32BE(nonce, 44);

        await zksyncContract.testProcessOperation(pubdata, "0x", [0]);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Change pubkey with posted signature", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const ethWitness = await zksync.utils.signChangePubkeyMessage(wallet, pubkeyHash, nonce);

        const accountId = 0xffee12;

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(8 * 6, 0);
        pubdata[0] = 0x07;
        pubdata.writeUIntBE(accountId, 1, 3);
        Buffer.from(pubkeyHash.substr(5), "hex").copy(pubdata, 4);
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, 24);
        pubdata.writeUInt32BE(nonce, 44);

        await zksyncContract.testProcessOperation(pubdata, ethWitness, [(ethWitness.length - 2) / 2]); // (ethWitness.length - 2) / 2   ==   len of ethWitness in bytes

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });
});
