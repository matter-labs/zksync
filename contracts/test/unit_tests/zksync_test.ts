import {
    addTestERC20Token, addTestNotApprovedERC20Token,
    deployFranklin,
    deployGovernance,
    deployVerifier, franklinTestContractCode,
    governanceTestContractCode, mintTestERC20Token,
    verifierTestContractCode
} from "../../src.ts/deploy";
import {BigNumber, bigNumberify, BigNumberish, parseEther} from "ethers/utils";
import {ETHProxy} from "zksync";
import {Address, TokenAddress} from "zksync/build/types";
import {AddressZero} from "ethers/constants";
import {Contract, ethers} from "ethers";

const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { wallet, exitWallet, deployTestContract, getCallRevertReason, IERC20_INTERFACE} = require("./common");
import * as zksync from "zksync";

const TEST_PRIORITY_EXPIRATION = 16;


describe("ZK Sync signature verification unit tests", function () {
    this.timeout(50000);

    let testContract;
    let randomWallet = ethers.Wallet.createRandom();
    before(async () => {
        testContract = await deployContract(wallet, require('../../build/ZKSyncUnitTest'), [AddressZero, AddressZero, AddressZero, Buffer.alloc(32, 0)], {
            gasLimit: 6000000,
        });
    });

    it("signature verification success", async () => {
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {revertReason, result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), nonce, randomWallet.address));
        expect(result).eq(true);
    });

    it("signature verification incorrect nonce", async () => {
        const incorrectNonce = 0x11223345;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), incorrectNonce, randomWallet.address));
        expect(result).eq(false);
    });

    it("signature verification incorrect pubkey hash", async () => {
        const incorrectPubkeyHash = "sync:aaaafefefefefefefefefefefefefefefefefefe";
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, incorrectPubkeyHash.replace("sync:", "0x"), nonce, randomWallet.address));
        expect(result).eq(false);
    });

    it("signature verification incorrect signer", async () => {
        const incorrectSignerAddress = wallet.address;
        const pubkeyHash = "sync:fefefefefefefefefefefefefefefefefefefefe";
        const nonce = 0x11223344;
        const signature = await zksync.utils.signChangePubkeyMessage(randomWallet, pubkeyHash, nonce);
        let {result} = await getCallRevertReason(() =>
            testContract.changePubkeySignatureCheck(signature, pubkeyHash.replace("sync:", "0x"), nonce, incorrectSignerAddress));
        expect(result).eq(false);
    });

});

describe("ZK priority queue ops unit tests", function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let ethProxy;
    let operationTestContract;
    before(async () => {
        const verifierDeployedContract = await deployVerifier(wallet, verifierTestContractCode, []);
        const governanceDeployedContract = await deployGovernance(wallet, governanceTestContractCode, [wallet.address]);
        zksyncContract = await deployFranklin(
            wallet,
            franklinTestContractCode,
            [
                governanceDeployedContract.address,
                verifierDeployedContract.address,
                wallet.address,
                ethers.constants.HashZero,
            ],
        );
        await governanceDeployedContract.setValidator(wallet.address, true);
        tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
        await mintTestERC20Token(wallet, tokenContract);
        ethProxy = new ETHProxy(wallet.provider, {mainContract: zksyncContract.address, govContract: governanceDeployedContract.address});

        operationTestContract = await deployTestContract('../../build/OperationsTest');
    });

    async function performDeposit(to: Address, token: TokenAddress, depositAmount: BigNumber, feeInEth: BigNumberish) {
        const openedRequests = await zksyncContract.totalOpenPriorityRequests();
        const depositOwner = wallet.address;

        let tx;
        if (token === ethers.constants.AddressZero) {
            tx = await zksyncContract.depositETH(depositAmount, depositOwner, {value: depositAmount.add(feeInEth)});
        } else {
            tx = await zksyncContract.depositERC20(token, depositAmount, depositOwner, {value: feeInEth});
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
        expect(priorityQueueEvent.values.fee.toString(), "request fee").eq(feeInEth.toString());
        expect(priorityQueueEvent.values.expirationBlock, "expiration block").eq(deadlineBlock);
        const parsedDepositPubdata = await operationTestContract.parseDepositFromPubdata(priorityQueueEvent.values.pubData);

        expect(parsedDepositPubdata.tokenId, "parsed token id").eq(await ethProxy.resolveTokenId(token));
        expect(parsedDepositPubdata.amount.toString(), "parsed amount").eq(depositAmount.toString());
        expect(parsedDepositPubdata.owner, "parsed owner").eq(depositOwner);
    }

    async function performFullExitRequest(accountId: number, token: TokenAddress, feeInEth: BigNumberish) {
        const openedRequests = await zksyncContract.totalOpenPriorityRequests();
        const tx = await zksyncContract.fullExit(accountId, token, {value: feeInEth});
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
        expect(priorityQueueEvent.values.fee.toString(), "request fee").eq(feeInEth.toString());
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
        const fee = await ethProxy.estimateDepositFeeInETHToken(tokenAddress);

        await performDeposit(wallet.address, tokenAddress, depositAmount, fee);
        await performDeposit(ethers.Wallet.createRandom().address, tokenAddress, depositAmount, fee);
    });

    it("success ERC20 deposits", async () => {
        zksyncContract.connect(wallet);
        const tokenAddress = tokenContract.address;
        const depositAmount = parseEther("1.0");
        const fee = await ethProxy.estimateDepositFeeInETHToken(tokenAddress);

        tokenContract.connect(wallet);
        await tokenContract.approve(zksyncContract.address, depositAmount);
        await performDeposit(wallet.address, tokenAddress, depositAmount, fee);
        await tokenContract.approve(zksyncContract.address, depositAmount);
        await performDeposit(ethers.Wallet.createRandom().address, tokenAddress, depositAmount, fee);
    });

    it("success FullExit request", async () => {
        zksyncContract.connect(wallet);
        const accountId = 1;
        const fee = await ethProxy.estimateEmergencyWithdrawFeeInETHToken();

        await performFullExitRequest(accountId, ethers.constants.AddressZero, fee);
        await performFullExitRequest(accountId, tokenContract.address, fee);
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

describe("ZK Sync withdraw unit tests", function () {
    this.timeout(50000);

    let zksyncContract;
    let tokenContract;
    let incorrectTokenContract;
    let ethProxy;
    before(async () => {
        const verifierDeployedContract = await deployVerifier(wallet, verifierTestContractCode, []);
        const governanceDeployedContract = await deployGovernance(wallet, governanceTestContractCode, [wallet.address]);
        zksyncContract = await deployFranklin(
            wallet,
            require("../../build/ZKSyncUnitTest"),
            [
                governanceDeployedContract.address,
                verifierDeployedContract.address,
                wallet.address,
                ethers.constants.HashZero,
            ],
        );
        await governanceDeployedContract.setValidator(wallet.address, true);
        tokenContract = await addTestERC20Token(wallet, governanceDeployedContract);
        incorrectTokenContract = await addTestNotApprovedERC20Token(wallet);
        await mintTestERC20Token(wallet, tokenContract);
        ethProxy = new ETHProxy(wallet.provider, {mainContract: zksyncContract.address, govContract: governanceDeployedContract.address});
    });

    async function performWithdraw(ethWallet: ethers.Wallet, token: TokenAddress, tokenId: number, amount: BigNumber) {
        let gasFee: BigNumber;
        const balanceBefore = await onchainBalance(ethWallet, token);
        const contractBalanceBefore = bigNumberify(await zksyncContract.balancesToWithdraw(ethWallet.address, tokenId));
        if (token === ethers.constants.AddressZero) {
            const tx = await zksyncContract.withdrawETH(amount);
            const receipt = await tx.wait();
            gasFee = receipt.gasUsed.mul(await ethWallet.provider.getGasPrice());
        } else {
            await zksyncContract.withdrawERC20(token, amount);
        }
        const balanceAfter = await onchainBalance(ethWallet, token);

        const expectedBalance = token == AddressZero ? balanceBefore.add(amount).sub(gasFee) : balanceBefore.add(amount);
        expect(balanceAfter.toString(), "withdraw account balance mismatch").eq(expectedBalance.toString());

        const contractBalanceAfter = bigNumberify(await zksyncContract.balancesToWithdraw(ethWallet.address, tokenId));
        const expectedContractBalance = contractBalanceBefore.sub(amount);
        expect(contractBalanceAfter.toString(), "withdraw contract balance mismatch").eq(expectedContractBalance.toString());
    }

    it("Withdraw ETH success", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const sendETH = await wallet.sendTransaction({to: zksyncContract.address, value: withdrawAmount.mul(2)});
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

        const sendETH = await wallet.sendTransaction({to: zksyncContract.address, value: withdrawAmount});
        await sendETH.wait();

        await zksyncContract.setBalanceToWithdraw(wallet.address, 0, withdrawAmount);
        const {revertReason} = await getCallRevertReason( async () => await performWithdraw(wallet, AddressZero, 0, withdrawAmount.add(1)));
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

        const {revertReason} = await getCallRevertReason( async () => await performWithdraw(wallet, tokenContract.address, tokenId, withdrawAmount.add(1)));
        expect(revertReason, "wrong revert reason").eq("frw11");
    });

    it("Withdraw ERC20 unsupported token", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");

        const {revertReason} = await getCallRevertReason( async () => await performWithdraw(wallet, incorrectTokenContract.address, 1, withdrawAmount.add(1)));
        expect(revertReason, "wrong revert reason").eq("gvs12");
    });

    it("Complete pending withdawals, eth, known erc20", async () => {
        zksyncContract.connect(wallet);
        const withdrawAmount = parseEther("1.0");
        const withdrawsToCancel = 5;

        await wallet.sendTransaction({to: zksyncContract.address, value: withdrawAmount});
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
