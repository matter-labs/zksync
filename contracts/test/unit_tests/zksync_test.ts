import { Contract, ethers, constants, BigNumber } from "ethers";
import { parseEther } from "ethers/lib/utils";
import { ETHProxy } from "zksync";
import { Address, TokenAddress } from "zksync/build/types";
import { Deployer, readContractCode, readTestContracts } from "../../src.ts/deploy";

const { simpleEncode } = require("ethereumjs-abi");
const { expect } = require("chai");
const { deployContract } = require("ethereum-waffle");
const { wallet, exitWallet, deployTestContract, getCallRevertReason, IERC20_INTERFACE } = require("./common");
import * as zksync from "zksync";

const TEST_PRIORITY_EXPIRATION = 101;
const CHUNK_SIZE = 9;

describe("zkSync signature verification unit tests", function() {
    this.timeout(50000);

    let testContract;
    const randomWallet = ethers.Wallet.createRandom();
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZKSyncSignatureUnitTest");
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        testContract = deployer.zkSyncContract(wallet);
    });

    it("pubkey hash signature verification success", async () => {
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const { revertReason, result } = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(
                signature,
                pubkeyHash.replace("sync:", "0x"),
                nonce,
                randomWallet.address,
                accountId
            )
        );
        expect(result).eq(true);
    });

    it("pubkey hash signature verification incorrect nonce", async () => {
        const incorrectNonce = 0x11223345;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const { result } = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(
                signature,
                pubkeyHash.replace("sync:", "0x"),
                incorrectNonce,
                randomWallet.address,
                accountId
            )
        );
        expect(result).eq(false);
    });

    it("pubkey hash signature verification incorrect pubkey hash", async () => {
        const incorrectPubkeyHash = "sync:aaaafefefefefefefefefefefefefefefefefefe";
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const { result } = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(
                signature,
                incorrectPubkeyHash.replace("sync:", "0x"),
                nonce,
                randomWallet.address,
                accountId
            )
        );
        expect(result).eq(false);
    });

    it("pubkey hash signature verification incorrect signer", async () => {
        const incorrectSignerAddress = wallet.address;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const { result } = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(
                signature,
                pubkeyHash.replace("sync:", "0x"),
                nonce,
                incorrectSignerAddress,
                accountId
            )
        );
        expect(result).eq(false);
    });

    it("pubkey hash signature verification incorrect account id", async () => {
        const incorrectAccountId = 0xbabeba;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const accountId = 0xdeadba;
        const signature = await randomWallet.signMessage(
            zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId)
        );
        const { result } = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(
                signature,
                pubkeyHash.replace("sync:", "0x"),
                nonce,
                randomWallet.address,
                incorrectAccountId
            )
        );
        expect(result).eq(false);
    });

    it("signature verification success", async () => {
        for (const message of [Buffer.from("msg", "ascii"), Buffer.alloc(0), Buffer.alloc(10, 1)]) {
            const signature = await wallet.signMessage(message);
            const sinedMessage = Buffer.concat([
                Buffer.from(`\x19Ethereum Signed Message:\n${message.length}`, "ascii"),
                message,
            ]);
            const address = await testContract.testRecoverAddressFromEthSignature(signature, sinedMessage);
            expect(address, `address mismatch, message ${message.toString("hex")}`).eq(wallet.address);
        }
    });
});

describe("ZK priority queue ops unit tests", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let operationTestContract;
    before(async () => {
        const contracts = readTestContracts();
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("TestnetERC20Token"),
            ["Matter Labs Trial Token", "MLTT", 18],
            { gasLimit: 5000000 }
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });

        operationTestContract = await deployTestContract("../../build/OperationsTest");
    });

    async function performDeposit(to: Address, token: TokenAddress, depositAmount: BigNumber) {
        const openedRequests = await zksyncContract.totalOpenPriorityRequests();
        const depositOwner = wallet.address;

        let tx;
        if (token === ethers.constants.AddressZero) {
            tx = await zksyncContract.depositETH(depositOwner, { value: depositAmount });
        } else {
            tx = await zksyncContract.depositERC20(token, depositAmount, depositOwner);
        }
        const receipt = await tx.wait();

        const deadlineBlock = receipt.blockNumber + TEST_PRIORITY_EXPIRATION;

        let priorityQueueEvent;
        for (const event of receipt.logs) {
            try {
                const parsedLog = zksyncContract.interface.parseLog(event);
                if (parsedLog && parsedLog.name === "NewPriorityRequest") {
                    priorityQueueEvent = parsedLog;
                    break;
                }
            } catch {}
        }
        expect(priorityQueueEvent.name, "event name").eq("NewPriorityRequest");
        expect(priorityQueueEvent.args.sender, "sender address").eq(wallet.address);
        expect(priorityQueueEvent.args.serialId, "request id").eq(openedRequests);
        expect(priorityQueueEvent.args.opType, "request type").eq(1);
        expect(priorityQueueEvent.args.expirationBlock, "expiration block").eq(deadlineBlock);
        const parsedDepositPubdata = await operationTestContract.parseDepositFromPubdata(
            priorityQueueEvent.args.pubData
        );

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
            try {
                const parsedLog = zksyncContract.interface.parseLog(event);
                if (parsedLog && parsedLog.name === "NewPriorityRequest") {
                    priorityQueueEvent = parsedLog;
                    break;
                }
            } catch {}
        }
        expect(priorityQueueEvent.name, "event name").eq("NewPriorityRequest");
        expect(priorityQueueEvent.args.sender, "sender address").eq(wallet.address);
        expect(priorityQueueEvent.args.serialId, "request id").eq(openedRequests);
        expect(priorityQueueEvent.args.opType, "request type").eq(6);
        expect(priorityQueueEvent.args.expirationBlock, "expiration block").eq(deadlineBlock);

        const parsedFullExitPubdata = await operationTestContract.parseFullExitFromPubdata(
            priorityQueueEvent.args.pubData
        );
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
        const erc20contract = new Contract(token, IERC20_INTERFACE.abi, ethWallet);
        return BigNumber.from(await erc20contract.balanceOf(ethWallet.address));
    }
}

describe("zkSync withdraw unit tests", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZkSyncWithdrawalUnitTest");
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("TestnetERC20Token"),
            ["Matter Labs Trial Token", "MLTT", 18],
            { gasLimit: 5000000 }
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });

        incorrectTokenContract = await deployContract(
            wallet,
            readContractCode("TestnetERC20Token"),
            ["Matter Labs Trial Token", "MLTT", 18],
            { gasLimit: 5000000 }
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));
    });

    async function performWithdraw(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        const contractBalanceBefore = BigNumber.from(
            await zksyncContract.getBalanceToWithdraw(ethWallet.address, tokenId)
        );
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawETH(amount, { gasLimit: 300000 });
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawERC20(token, amount, { gasLimit: 300000 });
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance =
            token == constants.AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), "withdraw account balance mismatch").eq(expectedBalance.toString());

        const contractBalanceAfter = BigNumber.from(
            await zksyncContract.getBalanceToWithdraw(ethWallet.address, tokenId)
        );
        const expectedContractBalance = contractBalanceBefore.sub(amount);
        expect(contractBalanceAfter.toString(), "withdraw contract balance mismatch").eq(
            expectedContractBalance.toString()
        );
    }

    it("Withdraw ETH success", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendETH = await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount.mul(2),
            data: simpleEncode("receiveETH()"),
        });
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount);

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount.div(2));
        await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount.div(2));
    });

    it("Withdraw ETH incorrect amount", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendETH = await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount,
            data: simpleEncode("receiveETH()"),
        });
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        const { revertReason } = await getCallRevertReason(
            async () => await performWithdraw(wallet, constants.AddressZero, 0, withdrawAmount.add(1))
        );
        expect(revertReason, "wrong revert reason").eq("SafeMath: subtraction overflow");
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

        const onchainBalBefore = await onchainBalance(wallet, tokenContract.address);
        const { revertReason } = await getCallRevertReason(
            async () => await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.add(1))
        );
        const onchainBalAfter = await onchainBalance(wallet, tokenContract.address);
        expect(onchainBalAfter).eq(onchainBalBefore);
    });

    it("Withdraw ERC20 unsupported token", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const { revertReason } = await getCallRevertReason(
            async () => await performWithdraw(wallet, incorrectTokenContract.address, 1, withdrawAmount.add(1))
        );
        expect(revertReason, "wrong revert reason").eq("gvs11");
    });

    it("Complete pending withdawals, eth, known erc20", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");
        const withdrawsToCancel = 5;

        await wallet.sendTransaction({
            to: zksyncContract.address,
            value: withdrawAmount,
            data: simpleEncode("receiveETH()"),
        });
        await tokenContract.transfer(zksyncContract.address, withdrawAmount);

        for (const tokenAddress of [constants.AddressZero, tokenContract.address]) {
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

describe("zkSync auth pubkey onchain unit tests", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    before(async () => {
        const contracts = readTestContracts();
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("TestnetERC20Token"),
            ["Matter Labs Trial Token", "MLTT", 18],
            { gasLimit: 5000000 }
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });
    });

    it("Auth pubkey success", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefe";

        const receipt = await (await zksyncContract.setAuthPubkeyHash(pubkeyHash, nonce)).wait();
        let authEvent;
        for (const event of receipt.logs) {
            try {
                const parsedLog = zksyncContract.interface.parseLog(event);
                if (parsedLog && parsedLog.name === "FactAuth") {
                    authEvent = parsedLog;
                    break;
                }
            } catch {}
        }

        expect(authEvent.args.sender, "event sender incorrect").eq(wallet.address);
        expect(authEvent.args.nonce, "event nonce incorrect").eq(nonce);
        expect(authEvent.args.fact, "event fact incorrect").eq(pubkeyHash);
    });

    it("Auth pubkey rewrite fail", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0xdead;
        const pubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefe";

        await zksyncContract.setAuthPubkeyHash(pubkeyHash, nonce, { gasLimit: 300000 });
        //
        const otherPubkeyHash = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const { revertReason } = await getCallRevertReason(
            async () => await zksyncContract.setAuthPubkeyHash(otherPubkeyHash, nonce, { gasLimit: 300000 })
        );
        expect(revertReason, "revert reason incorrect").eq("ahf11");
    });

    it("Auth pubkey incorrect length fail", async () => {
        zksyncContract.connect(wallet);
        const nonce = 0x7656;
        const shortPubkeyHash = "0xfefefefefefefefefefefefefefefefefefefe";
        const longPubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefefe";

        for (const pkHash of [shortPubkeyHash, longPubkeyHash]) {
            const { revertReason } = await getCallRevertReason(
                async () => await zksyncContract.setAuthPubkeyHash(shortPubkeyHash, nonce, { gasLimit: 300000 })
            );
            expect(revertReason, "revert reason incorrect").eq("ahf10");
        }
    });
});

describe("zkSync test process next operation", function() {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;
    before(async () => {
        const contracts = readTestContracts();
        contracts.zkSync = readContractCode("ZkSyncProcessOpUnitTest");
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployAll({ gasLimit: 6500000 });
        zksyncContract = deployer.zkSyncContract(wallet);

        tokenContract = await deployContract(
            wallet,
            readContractCode("TestnetERC20Token"),
            ["Matter Labs Trial Token", "MLTT", 18],
            { gasLimit: 5000000 }
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));

        const govContract = deployer.governanceContract(wallet);
        await govContract.addToken(tokenContract.address);

        ethProxy = new ETHProxy(wallet.provider, {
            mainContract: zksyncContract.address,
            govContract: govContract.address,
        });

        incorrectTokenContract = await deployContract(
            wallet,
            readContractCode("TestnetERC20Token"),
            ["Matter Labs Trial Token", "MLTT", 18],
            { gasLimit: 5000000 }
        );
        await tokenContract.mint(wallet.address, parseEther("1000000"));
    });

    it("Process noop", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(CHUNK_SIZE, 0);
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process transfer", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(CHUNK_SIZE * 2, 0xff);
        pubdata[0] = 0x05;
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });
    it("Process transfer to new", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0xff);
        pubdata[0] = 0x02;
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process deposit", async () => {
        zksyncContract.connect(wallet);
        const depositAmount = BigNumber.from("2");

        await zksyncContract.depositETH(wallet.address, { value: depositAmount });

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x01;
        let offset = 1;
        pubdata.writeUInt32BE(0xccaabbff, offset);
        offset += 4;
        pubdata.writeUInt16BE(0, offset); // token
        offset += 2;
        Buffer.from(
            depositAmount
                .toHexString()
                .substr(2)
                .padStart(16 * 2, "0"),
            "hex"
        ).copy(pubdata, offset);
        offset += 16;
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, offset);
        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter - 1, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process partial exit", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x03;

        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process full exit", async () => {
        zksyncContract.connect(wallet);
        const tokenId = await ethProxy.resolveTokenId(tokenContract.address);
        const fullExitAmount = parseEther("0.7");
        const accountId = 0x00ffffff;

        await zksyncContract.fullExit(accountId, tokenContract.address);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct full exit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x06;
        let offset = 1;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt16BE(tokenId, offset);
        offset += 2;
        Buffer.from(
            fullExitAmount
                .toHexString()
                .substr(2)
                .padStart(16 * 2, "0"),
            "hex"
        ).copy(pubdata, offset);

        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter - 1, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Change pubkey with auth", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = "0xfefefefefefefefefefefefefefefefefefefefe";
        await zksyncContract.setAuthPubkeyHash(pubkeyHash, nonce);

        const accountId = 0xffee12cc;

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x07;
        let offset = 1;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(pubkeyHash.substr(2), "hex").copy(pubdata, offset);
        offset += 20;
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt32BE(nonce, offset);

        await zksyncContract.testProcessOperation(pubdata, "0x", [0]);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Change pubkey with posted signature", async () => {
        zksyncContract.connect(wallet);

        const nonce = 0x1234;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const accountId = 0x00ffee12;
        const ethWitness = await wallet.signMessage(zksync.utils.getChangePubkeyMessage(pubkeyHash, nonce, accountId));

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x07;
        let offset = 1;
        pubdata.writeUInt32BE(accountId, offset);
        offset += 4;
        Buffer.from(pubkeyHash.substr(5), "hex").copy(pubdata, offset);
        offset += 20;
        Buffer.from(wallet.address.substr(2), "hex").copy(pubdata, offset);
        offset += 20;
        pubdata.writeUInt32BE(nonce, offset);

        await zksyncContract.testProcessOperation(pubdata, ethWitness, [(ethWitness.length - 2) / 2]); // (ethWitness.length - 2) / 2   ==   len of ethWitness in bytes

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });

    it("Process forced exit", async () => {
        zksyncContract.connect(wallet);

        const committedPriorityRequestsBefore = await zksyncContract.totalCommittedPriorityRequests();

        // construct deposit pubdata
        const pubdata = Buffer.alloc(CHUNK_SIZE * 6, 0);
        pubdata[0] = 0x08;

        await zksyncContract.testProcessOperation(pubdata, "0x", []);

        const committedPriorityRequestsAfter = await zksyncContract.totalCommittedPriorityRequests();
        expect(committedPriorityRequestsAfter, "priority request number").eq(committedPriorityRequestsBefore);
    });
});
