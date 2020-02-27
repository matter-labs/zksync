import {
    addTestERC20Token,
    deployFranklin,
    deployGovernance,
    deployVerifier, franklinTestContractCode,
    governanceTestContractCode, mintTestERC20Token,
    verifierTestContractCode
} from "../../src.ts/deploy";
import {BigNumber, BigNumberish, parseEther} from "ethers/utils";
import {ETHProxy} from "zksync";
import {Address, TokenAddress} from "zksync/build/types";

const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { wallet, deployTestContract, getCallRevertReason } = require("./common")
const {ethers } = require("ethers");
const zksync = require("zksync");

const TEST_PRIORITY_EXPIRATION = 16;


describe("ZK Sync signature verification unit tests", function () {
    this.timeout(50000);

    let testContract;
    let randomWallet = ethers.Wallet.createRandom();
    before(async () => {
        testContract = await deployContract(wallet, require('../../build/ZKSyncUnitTest'), [], {
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

describe("ZK Sync deposit unit tests", function () {
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
